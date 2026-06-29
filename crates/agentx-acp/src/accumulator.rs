//! Folds the ACP streaming-update protocol into complete domain events.
//!
//! ACP delivers a turn as many fine-grained [`acp::SessionUpdate`]s: message
//! text arrives as a run of chunks, a tool call arrives as a `ToolCall` start
//! followed by `ToolCallUpdate` deltas. The domain timeline
//! ([`SessionEvent`]) is coarser — one event per *complete* message, thought,
//! tool call, or plan. [`StreamAccumulator`] is the stateful transducer that
//! bridges the two: feed it each update with [`push`](StreamAccumulator::push),
//! call [`finish`](StreamAccumulator::finish) when the turn ends, and it yields
//! the domain events ready to persist and publish.
//!
//! Two rules keep the fold predictable:
//! 1. **Text runs coalesce.** Consecutive message (or thought) chunks merge
//!    into one event. A change of kind, or any non-text update, closes the run.
//! 2. **Tool calls fold to a single event.** The start and its updates merge
//!    into one snapshot, emitted when the call reaches a terminal status
//!    (completed/failed) — or at [`finish`](StreamAccumulator::finish) if it
//!    never does.

use std::mem;

use agent_client_protocol::schema as acp;
use agentx_domain::{ContentBlock, SessionEvent, ToolCall, ToolCallStatus};

use crate::mapping::{
    content_block_to_domain, plan_to_domain, tool_call_content_to_domain,
    tool_call_status_to_domain, tool_call_to_domain, tool_kind_to_domain,
};

/// Which kind of text run is currently open.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TextRun {
    AgentMessage,
    AgentThought,
}

/// Accumulates one turn's ACP updates into domain [`SessionEvent`]s.
///
/// Reusable across turns: [`finish`](Self::finish) drains and clears all state.
#[derive(Default)]
pub(crate) struct StreamAccumulator {
    /// The open text run, if message/thought chunks are currently buffering.
    run: Option<TextRun>,
    /// Content of the open text run.
    blocks: Vec<ContentBlock>,
    /// Tool calls seen but not yet terminal, in arrival order.
    tools: Vec<ToolCall>,
}

impl StreamAccumulator {
    /// Fold one update in, returning any events it completed.
    pub(crate) fn push(&mut self, update: acp::SessionUpdate) -> Vec<SessionEvent> {
        let mut out = Vec::new();
        match update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                self.append_text(TextRun::AgentMessage, chunk.content, &mut out);
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                self.append_text(TextRun::AgentThought, chunk.content, &mut out);
            }
            acp::SessionUpdate::UserMessageChunk(chunk) => {
                // Each user chunk is a distinct block (text, a code selection, an
                // image) kept separate for rendering, so it is emitted as its own
                // message rather than merged.
                self.flush_text(&mut out);
                if let Some(block) = content_block_to_domain(chunk.content) {
                    out.push(SessionEvent::UserMessage {
                        content: vec![block],
                    });
                }
            }
            acp::SessionUpdate::ToolCall(call) => {
                self.flush_text(&mut out);
                self.start_tool_call(tool_call_to_domain(call), &mut out);
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                self.flush_text(&mut out);
                self.update_tool_call(update, &mut out);
            }
            acp::SessionUpdate::Plan(plan) => {
                self.flush_text(&mut out);
                out.push(SessionEvent::Plan(plan_to_domain(plan)));
            }
            // Mode/command/config/usage updates are not part of the domain
            // timeline, but they still close any open text run.
            _ => self.flush_text(&mut out),
        }
        out
    }

    /// End the turn: flush any open text run and emit every tool call that
    /// never reached a terminal status. Leaves the accumulator empty.
    pub(crate) fn finish(&mut self) -> Vec<SessionEvent> {
        let mut out = Vec::new();
        self.flush_text(&mut out);
        for call in mem::take(&mut self.tools) {
            out.push(SessionEvent::ToolCall(call));
        }
        out
    }

    fn append_text(
        &mut self,
        kind: TextRun,
        block: acp::ContentBlock,
        out: &mut Vec<SessionEvent>,
    ) {
        if self.run != Some(kind) {
            self.flush_text(out);
            self.run = Some(kind);
        }
        if let Some(block) = content_block_to_domain(block) {
            push_merged_text(&mut self.blocks, block);
        }
    }

    fn flush_text(&mut self, out: &mut Vec<SessionEvent>) {
        let Some(kind) = self.run.take() else {
            return;
        };
        let blocks = mem::take(&mut self.blocks);
        if blocks.is_empty() {
            return;
        }
        out.push(match kind {
            TextRun::AgentMessage => SessionEvent::AgentMessage { content: blocks },
            TextRun::AgentThought => SessionEvent::AgentThought {
                text: blocks
                    .iter()
                    .filter_map(ContentBlock::as_plain_text)
                    .collect(),
            },
        });
    }

    fn start_tool_call(&mut self, call: ToolCall, out: &mut Vec<SessionEvent>) {
        if is_terminal(call.status) {
            out.push(SessionEvent::ToolCall(call));
        } else if let Some(slot) = self.tools.iter_mut().find(|t| t.id == call.id) {
            *slot = call;
        } else {
            self.tools.push(call);
        }
    }

    fn update_tool_call(&mut self, update: acp::ToolCallUpdate, out: &mut Vec<SessionEvent>) {
        let id = update.tool_call_id.to_string();
        if let Some(pos) = self.tools.iter().position(|t| t.id == id) {
            apply_update_fields(&mut self.tools[pos], update.fields);
            if is_terminal(self.tools[pos].status) {
                out.push(SessionEvent::ToolCall(self.tools.remove(pos)));
            }
        } else if let Some(call) = tool_call_from_update(id, update.fields) {
            self.start_tool_call(call, out);
        }
    }
}

/// Append a block, merging into the trailing block when both are text so a run
/// of streamed text chunks collapses into one block.
fn push_merged_text(blocks: &mut Vec<ContentBlock>, block: ContentBlock) {
    if let (ContentBlock::Text(addition), Some(ContentBlock::Text(existing))) =
        (&block, blocks.last_mut())
    {
        existing.push_str(addition);
    } else {
        blocks.push(block);
    }
}

fn is_terminal(status: ToolCallStatus) -> bool {
    matches!(status, ToolCallStatus::Completed | ToolCallStatus::Failed)
}

/// Apply an ACP update onto an existing tool-call snapshot. Absent fields leave
/// the current value untouched; `content` replaces (never extends).
fn apply_update_fields(call: &mut ToolCall, fields: acp::ToolCallUpdateFields) {
    if let Some(title) = fields.title {
        call.title = title;
    }
    if let Some(kind) = fields.kind {
        call.kind = tool_kind_to_domain(kind);
    }
    if let Some(status) = fields.status {
        call.status = tool_call_status_to_domain(status);
    }
    if let Some(content) = fields.content {
        call.content = content
            .into_iter()
            .filter_map(tool_call_content_to_domain)
            .collect();
    }
}

/// Seed a snapshot from an update that arrived with no prior `ToolCall` start.
/// A title is required to form a domain tool call; without one the update is
/// dropped (returns `None`).
fn tool_call_from_update(id: String, fields: acp::ToolCallUpdateFields) -> Option<ToolCall> {
    Some(ToolCall {
        id,
        title: fields.title?,
        kind: fields.kind.map(tool_kind_to_domain).unwrap_or_default(),
        status: fields
            .status
            .map(tool_call_status_to_domain)
            .unwrap_or_default(),
        content: fields
            .content
            .unwrap_or_default()
            .into_iter()
            .filter_map(tool_call_content_to_domain)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentx_domain::ToolKind;

    fn agent_chunk(text: &str) -> acp::SessionUpdate {
        acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk::new(text.into()))
    }

    fn thought_chunk(text: &str) -> acp::SessionUpdate {
        acp::SessionUpdate::AgentThoughtChunk(acp::ContentChunk::new(text.into()))
    }

    fn user_chunk(text: &str) -> acp::SessionUpdate {
        acp::SessionUpdate::UserMessageChunk(acp::ContentChunk::new(text.into()))
    }

    fn tool_start(
        id: &'static str,
        title: &str,
        status: acp::ToolCallStatus,
    ) -> acp::SessionUpdate {
        acp::SessionUpdate::ToolCall(acp::ToolCall::new(id, title.to_string()).status(status))
    }

    fn tool_update(id: &'static str, status: acp::ToolCallStatus) -> acp::SessionUpdate {
        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            id,
            acp::ToolCallUpdateFields::new().status(status),
        ))
    }

    #[test]
    fn consecutive_message_chunks_merge_into_one_event() {
        let mut acc = StreamAccumulator::default();
        assert!(acc.push(agent_chunk("Hello, ")).is_empty());
        assert!(acc.push(agent_chunk("world")).is_empty());
        assert_eq!(
            acc.finish(),
            vec![SessionEvent::AgentMessage {
                content: vec![ContentBlock::text("Hello, world")],
            }]
        );
    }

    #[test]
    fn thought_chunks_merge_into_one_thought() {
        let mut acc = StreamAccumulator::default();
        acc.push(thought_chunk("step one. "));
        acc.push(thought_chunk("step two."));
        assert_eq!(
            acc.finish(),
            vec![SessionEvent::AgentThought {
                text: "step one. step two.".into(),
            }]
        );
    }

    #[test]
    fn changing_run_kind_flushes_the_previous_run() {
        let mut acc = StreamAccumulator::default();
        acc.push(agent_chunk("answer"));
        // Switching to a thought closes the message run immediately.
        assert_eq!(
            acc.push(thought_chunk("hmm")),
            vec![SessionEvent::AgentMessage {
                content: vec![ContentBlock::text("answer")],
            }]
        );
        assert_eq!(
            acc.finish(),
            vec![SessionEvent::AgentThought { text: "hmm".into() }]
        );
    }

    #[test]
    fn user_chunk_emits_immediately_after_flushing_pending_text() {
        let mut acc = StreamAccumulator::default();
        acc.push(agent_chunk("partial"));
        assert_eq!(
            acc.push(user_chunk("hi")),
            vec![
                SessionEvent::AgentMessage {
                    content: vec![ContentBlock::text("partial")],
                },
                SessionEvent::UserMessage {
                    content: vec![ContentBlock::text("hi")],
                },
            ]
        );
    }

    #[test]
    fn tool_call_folds_start_and_update_into_one_event_on_completion() {
        let mut acc = StreamAccumulator::default();
        // Start is buffered, not emitted, while non-terminal.
        assert!(
            acc.push(tool_start("t1", "Edit file", acp::ToolCallStatus::Pending))
                .is_empty()
        );
        let events = acc.push(tool_update("t1", acp::ToolCallStatus::Completed));
        assert_eq!(events.len(), 1);
        let SessionEvent::ToolCall(call) = &events[0] else {
            panic!("expected a ToolCall event");
        };
        assert_eq!(call.id, "t1");
        // Title carried over from the start; status advanced by the update.
        assert_eq!(call.title, "Edit file");
        assert_eq!(call.status, ToolCallStatus::Completed);
    }

    #[test]
    fn tool_call_already_terminal_emits_immediately() {
        let mut acc = StreamAccumulator::default();
        let events = acc.push(tool_start("t1", "Quick", acp::ToolCallStatus::Completed));
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], SessionEvent::ToolCall(_)));
    }

    #[test]
    fn plan_flushes_text_then_emits_plan() {
        let mut acc = StreamAccumulator::default();
        acc.push(agent_chunk("done"));
        let plan = acp::SessionUpdate::Plan(acp::Plan::new(vec![acp::PlanEntry::new(
            "task",
            acp::PlanEntryPriority::Medium,
            acp::PlanEntryStatus::Pending,
        )]));
        let events = acc.push(plan);
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], SessionEvent::AgentMessage { .. }));
        assert!(matches!(events[1], SessionEvent::Plan(_)));
    }

    #[test]
    fn finish_drains_a_tool_call_that_never_completed() {
        let mut acc = StreamAccumulator::default();
        acc.push(tool_start("t1", "Running", acp::ToolCallStatus::InProgress));
        let events = acc.finish();
        assert_eq!(events.len(), 1);
        let SessionEvent::ToolCall(call) = &events[0] else {
            panic!("expected a ToolCall event");
        };
        assert_eq!(call.id, "t1");
        assert_eq!(call.kind, ToolKind::Other);
    }

    #[test]
    fn finish_resets_state_for_reuse() {
        let mut acc = StreamAccumulator::default();
        acc.push(agent_chunk("first turn"));
        assert_eq!(acc.finish().len(), 1);
        // A second turn on the same accumulator starts clean.
        assert!(acc.finish().is_empty());
    }
}
