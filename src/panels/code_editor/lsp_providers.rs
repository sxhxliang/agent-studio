use std::{ops::Range, str::FromStr, time::Duration};

use anyhow::anyhow;
use gpui::{App, AppContext, Context, Entity, Result, SharedString, Task, Window};
use gpui_component::{
    WindowExt,
    input::{
        CodeActionProvider, CompletionProvider, DefinitionProvider, DocumentColorProvider,
        HoverProvider, InputState, Rope, RopeExt,
    },
    notification::Notification,
};
use lsp_types::{
    CodeAction, CodeActionKind, CompletionContext, CompletionResponse, TextEdit, WorkspaceEdit,
};

use crate::AppState;

use super::lsp_store::CodeEditorPanelLspStore;
use super::types::{RUST_DOC_URLS, completion_item};

// ============================================================================
// CompletionProvider Implementation
// ============================================================================

impl CompletionProvider for CodeEditorPanelLspStore {
    fn completions(
        &self,
        rope: &Rope,
        offset: usize,
        trigger: CompletionContext,
        _: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<Result<CompletionResponse>> {
        let trigger_character = trigger.trigger_character.unwrap_or_default();
        if trigger_character.is_empty() {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        }

        // Simulate to delay for fetching completions
        let rope = rope.clone();
        let items = self.completions.clone();
        AppContext::background_spawn(cx, async move {
            // Simulate a slow completion source, to test Editor async handling.
            smol::Timer::after(Duration::from_millis(20)).await;

            if trigger_character.starts_with("/") {
                let start = offset.saturating_sub(trigger_character.len());
                let start_pos = rope.offset_to_position(start);
                let end_pos = rope.offset_to_position(offset);
                let replace_range = lsp_types::Range::new(start_pos, end_pos);

                let items = vec![
                    completion_item(
                        &replace_range,
                        "/date",
                        format!("{}", chrono::Local::now().date_naive()).as_str(),
                        "Insert current date",
                    ),
                    completion_item(&replace_range, "/thanks", "Thank you!", "Insert Thank you!"),
                    completion_item(&replace_range, "/+1", "👍", "Insert 👍"),
                    completion_item(&replace_range, "/-1", "👎", "Insert 👎"),
                    completion_item(&replace_range, "/smile", "😊", "Insert 😊"),
                    completion_item(&replace_range, "/sad", "😢", "Insert 😢"),
                    completion_item(&replace_range, "/launch", "🚀", "Insert 🚀"),
                ];
                return Ok(CompletionResponse::Array(items));
            }

            let items = items
                .iter()
                .filter(|item| item.label.starts_with(&trigger_character))
                .take(10)
                .map(|item| {
                    let mut item = item.clone();
                    item.insert_text = Some(item.label.replace(&trigger_character, ""));
                    item
                })
                .collect::<Vec<_>>();

            Ok(CompletionResponse::Array(items))
        })
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        _new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        true
    }
}

// ============================================================================
// CodeActionProvider Implementation for LspStore
// ============================================================================

impl CodeActionProvider for CodeEditorPanelLspStore {
    fn id(&self) -> SharedString {
        "LspStore".into()
    }

    fn code_actions(
        &self,
        _state: Entity<InputState>,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<Result<Vec<CodeAction>>> {
        let mut actions = vec![];
        for (node_range, code_action) in self.code_actions().iter() {
            if !(range.start >= node_range.start && range.end <= node_range.end) {
                continue;
            }

            actions.push(code_action.clone());
        }

        Task::ready(Ok(actions))
    }

    fn perform_code_action(
        &self,
        state: Entity<InputState>,
        action: CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let Some(edit) = action.edit else {
            return Task::ready(Ok(()));
        };

        let changes = if let Some(changes) = edit.changes {
            changes
        } else {
            return Task::ready(Ok(()));
        };

        let Some((_, text_edits)) = changes.into_iter().next() else {
            return Task::ready(Ok(()));
        };

        let state = state.downgrade();
        window.spawn(cx, async move |cx| {
            state.update_in(cx, |state, window, cx| {
                state.apply_lsp_edits(&text_edits, window, cx);
            })
        })
    }
}

// ============================================================================
// HoverProvider Implementation
// ============================================================================

impl HoverProvider for CodeEditorPanelLspStore {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<Result<Option<lsp_types::Hover>>> {
        let word = text.word_at(offset);
        if word.is_empty() {
            return Task::ready(Ok(None));
        }

        let Some(item) = self.completions.iter().find(|item| item.label == word) else {
            return Task::ready(Ok(None));
        };

        let contents = if let Some(doc) = &item.documentation {
            match doc {
                lsp_types::Documentation::String(s) => s.clone(),
                lsp_types::Documentation::MarkupContent(mc) => mc.value.clone(),
            }
        } else {
            "No documentation available.".to_string()
        };

        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(contents)),
            range: None,
        };

        Task::ready(Ok(Some(hover)))
    }
}

// ============================================================================
// DefinitionProvider Implementation
// ============================================================================

impl DefinitionProvider for CodeEditorPanelLspStore {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<Result<Vec<lsp_types::LocationLink>>> {
        let Some(word_range) = text.word_range(offset) else {
            return Task::ready(Ok(vec![]));
        };
        let word = text.slice(word_range.clone()).to_string();

        let document_uri = lsp_types::Uri::from_str("file://CodeEditorPanel").unwrap();
        let start = text.offset_to_position(word_range.start);
        let end = text.offset_to_position(word_range.end);
        let symbol_range = lsp_types::Range { start, end };

        if word == "Duration" {
            let target_range = lsp_types::Range {
                start: lsp_types::Position {
                    line: 2,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 2,
                    character: 23,
                },
            };
            return Task::ready(Ok(vec![lsp_types::LocationLink {
                target_uri: document_uri,
                target_range: target_range,
                target_selection_range: target_range,
                origin_selection_range: Some(symbol_range),
            }]));
        }

        let names = RUST_DOC_URLS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        for (ix, t) in names.iter().enumerate() {
            if *t == word {
                let url = RUST_DOC_URLS[ix].1;
                let location = lsp_types::LocationLink {
                    target_uri: lsp_types::Uri::from_str(&format!(
                        "https://doc.rust-lang.org/std/{}.html",
                        url
                    ))
                    .unwrap(),
                    target_selection_range: lsp_types::Range::default(),
                    target_range: lsp_types::Range::default(),
                    origin_selection_range: Some(symbol_range),
                };

                return Task::ready(Ok(vec![location]));
            }
        }

        Task::ready(Ok(vec![]))
    }
}

// ============================================================================
// DocumentColorProvider Implementation
// ============================================================================

impl DocumentColorProvider for CodeEditorPanelLspStore {
    fn document_colors(
        &self,
        text: &Rope,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<gpui::Result<Vec<lsp_types::ColorInformation>>> {
        let nodes = color_lsp::parse(&text.to_string());
        let colors = nodes
            .into_iter()
            .map(|node| {
                let start = lsp_types::Position::new(node.position.line, node.position.character);
                let end = lsp_types::Position::new(
                    node.position.line,
                    node.position.character + node.matched.chars().count() as u32,
                );

                lsp_types::ColorInformation {
                    range: lsp_types::Range { start, end },
                    color: lsp_types::Color {
                        red: node.color.r,
                        green: node.color.g,
                        blue: node.color.b,
                        alpha: node.color.a,
                    },
                }
            })
            .collect::<Vec<_>>();

        Task::ready(Ok(colors))
    }
}

// ============================================================================
// TextConvertor - Additional CodeActionProvider
// ============================================================================

pub struct TextConvertor;

impl CodeActionProvider for TextConvertor {
    fn id(&self) -> SharedString {
        "TextConvertor".into()
    }

    fn code_actions(
        &self,
        state: Entity<InputState>,
        range: Range<usize>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Vec<CodeAction>>> {
        let mut actions = vec![];
        if range.is_empty() {
            return Task::ready(Ok(actions));
        }

        let state = state.read(cx);
        let document_uri = lsp_types::Uri::from_str("file://CodeEditorPanel").unwrap();

        let old_text = state.text().slice(range.clone()).to_string();
        let start = state.text().offset_to_position(range.start);
        let end = state.text().offset_to_position(range.end);
        let range = lsp_types::Range { start, end };

        // actions.push(CodeAction {
        //     title: "Convert to Uppercase".into(),
        //     kind: Some(CodeActionKind::REFACTOR),
        //     edit: Some(WorkspaceEdit {
        //         changes: Some(
        //             std::iter::once((
        //                 document_uri.clone(),
        //                 vec![TextEdit {
        //                     range,
        //                     new_text: old_text.to_uppercase(),
        //                     ..Default::default()
        //                 }],
        //             ))
        //             .collect(),
        //         ),
        //         ..Default::default()
        //     }),
        //     ..Default::default()
        // });

        // actions.push(CodeAction {
        //     title: "Convert to Lowercase".into(),
        //     kind: Some(CodeActionKind::REFACTOR),
        //     edit: Some(WorkspaceEdit {
        //         changes: Some(
        //             std::iter::once((
        //                 document_uri.clone(),
        //                 vec![TextEdit {
        //                     range: range,
        //                     new_text: old_text.to_lowercase(),
        //                     ..Default::default()
        //                 }],
        //             ))
        //             .collect(),
        //         ),
        //         ..Default::default()
        //     }),
        //     ..Default::default()
        // });

        // actions.push(CodeAction {
        //     title: "Titleize".into(),
        //     kind: Some(CodeActionKind::REFACTOR),
        //     edit: Some(WorkspaceEdit {
        //         changes: Some(
        //             std::iter::once((
        //                 document_uri.clone(),
        //                 vec![TextEdit {
        //                     range: range,
        //                     new_text: old_text
        //                         .split_whitespace()
        //                         .map(|word| {
        //                             let mut chars = word.chars();
        //                             chars
        //                                 .next()
        //                                 .map(|c| c.to_uppercase().collect::<String>())
        //                                 .unwrap_or_default()
        //                                 + chars.as_str()
        //                         })
        //                         .collect::<Vec<_>>()
        //                         .join(" "),
        //                     ..Default::default()
        //                 }],
        //             ))
        //             .collect(),
        //         ),
        //         ..Default::default()
        //     }),
        //     ..Default::default()
        // });

        // actions.push(CodeAction {
        //     title: "Capitalize".into(),
        //     kind: Some(CodeActionKind::REFACTOR),
        //     edit: Some(WorkspaceEdit {
        //         changes: Some(
        //             std::iter::once((
        //                 document_uri.clone(),
        //                 vec![TextEdit {
        //                     range,
        //                     new_text: old_text
        //                         .chars()
        //                         .enumerate()
        //                         .map(|(i, c)| {
        //                             if i == 0 {
        //                                 c.to_uppercase().to_string()
        //                             } else {
        //                                 c.to_string()
        //                             }
        //                         })
        //                         .collect(),
        //                     ..Default::default()
        //                 }],
        //             ))
        //             .collect(),
        //         ),
        //         ..Default::default()
        //     }),
        //     ..Default::default()
        // });

        // // snake_case
        // actions.push(CodeAction {
        //     title: "Convert to snake_case".into(),
        //     kind: Some(CodeActionKind::REFACTOR),
        //     edit: Some(WorkspaceEdit {
        //         changes: Some(
        //             std::iter::once((
        //                 document_uri.clone(),
        //                 vec![TextEdit {
        //                     range,
        //                     new_text: old_text
        //                         .chars()
        //                         .enumerate()
        //                         .map(|(i, c)| {
        //                             if c.is_uppercase() {
        //                                 if i != 0 {
        //                                     format!("_{}", c.to_lowercase())
        //                                 } else {
        //                                     c.to_lowercase().to_string()
        //                                 }
        //                             } else {
        //                                 c.to_string()
        //                             }
        //                         })
        //                         .collect(),
        //                     ..Default::default()
        //                 }],
        //             ))
        //             .collect(),
        //         ),
        //         ..Default::default()
        //     }),
        //     ..Default::default()
        // });

        // AI-Powered Actions (only show if AI service is configured)
        if let Some(_ai_service) = AppState::global(cx).ai_service() {
            actions.push(CodeAction {
                title: "Add Documentation Comment (AI)".into(),
                kind: Some(CodeActionKind::REFACTOR),
                data: Some(serde_json::json!({
                    "ai_action": "doc_comment",
                    "code": old_text,
                    "range": range,
                })),
                ..Default::default()
            });

            actions.push(CodeAction {
                title: "Add Inline Comment (AI)".into(),
                kind: Some(CodeActionKind::REFACTOR),
                data: Some(serde_json::json!({
                    "ai_action": "inline_comment",
                    "code": old_text,
                    "range": range,
                })),
                ..Default::default()
            });

            actions.push(CodeAction {
                title: "Explain Code (AI)".into(),
                kind: Some(CodeActionKind::REFACTOR),
                data: Some(serde_json::json!({
                    "ai_action": "explain",
                    "code": old_text,
                })),
                ..Default::default()
            });

            actions.push(CodeAction {
                title: "Suggest Improvements (AI)".into(),
                kind: Some(CodeActionKind::REFACTOR),
                data: Some(serde_json::json!({
                    "ai_action": "improve",
                    "code": old_text,
                })),
                ..Default::default()
            });
        }

        Task::ready(Ok(actions))
    }

    fn perform_code_action(
        &self,
        state: Entity<InputState>,
        action: CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        // Check for AI actions first
        if let Some(data) = &action.data {
            if let Ok(json) = serde_json::from_value::<serde_json::Value>(data.clone()) {
                if json.get("ai_action").and_then(|v| v.as_str()).is_some() {
                    return self.perform_ai_action(state, json, window, cx);
                }
            }
        }

        // Existing text transformation logic
        let Some(edit) = action.edit else {
            return Task::ready(Ok(()));
        };

        let changes = if let Some(changes) = edit.changes {
            changes
        } else {
            return Task::ready(Ok(()));
        };

        let Some((_, text_edits)) = changes.into_iter().next() else {
            return Task::ready(Ok(()));
        };

        let state = state.downgrade();
        window.spawn(cx, async move |cx| {
            state.update_in(cx, |state, window, cx| {
                state.apply_lsp_edits(&text_edits, window, cx);
            })
        })
    }
}

impl TextConvertor {
    fn perform_ai_action(
        &self,
        state: Entity<InputState>,
        data: serde_json::Value,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<()>> {
        use crate::core::services::CommentStyle;

        let Some(ai_service) = AppState::global(cx).ai_service() else {
            // Show error notification if AI service is not configured
            struct AiServiceError;
            let note =
                Notification::error("AI service not configured. Please check your config.json")
                    .id::<AiServiceError>();
            window.push_notification(note, cx);
            return Task::ready(Err(anyhow!("AI service not configured")));
        };

        let ai_action = data
            .get("ai_action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let code = data
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let ai_service = ai_service.clone();
        let state_weak = state.downgrade();

        match ai_action.as_str() {
            "doc_comment" | "inline_comment" => {
                let range: lsp_types::Range = match data.get("range") {
                    Some(r) => serde_json::from_value(r.clone()).unwrap(),
                    None => {
                        return Task::ready(Err(anyhow!(
                            "Missing range data for AI comment action"
                        )));
                    }
                };

                let style = if ai_action == "doc_comment" {
                    CommentStyle::FunctionDoc
                } else {
                    CommentStyle::Inline
                };

                // Show loading notification
                struct AiCommentLoading;
                let loading_note =
                    Notification::info("Generating comment with AI...").id::<AiCommentLoading>();
                window.push_notification(loading_note, cx);

                window.spawn(cx, async move |cx| {
                    // Call AI service
                    let comment_result = ai_service.generate_comment(&code, style).await;

                    match comment_result {
                        Ok(comment) => {
                            let formatted = format_comment_for_code(&code, &comment, style);

                            state_weak.update_in(cx, |state, window, cx| {
                                state.apply_lsp_edits(
                                    &vec![TextEdit {
                                        range,
                                        new_text: formatted,
                                        ..Default::default()
                                    }],
                                    window,
                                    cx,
                                );

                                // Show success notification
                                struct AiCommentSuccess;
                                let success_note =
                                    Notification::success("Comment generated successfully!")
                                        .id::<AiCommentSuccess>();
                                window.push_notification(success_note, cx);
                            })?;

                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to generate comment: {}", e);

                            // Show error notification
                            struct AiCommentError;
                            log::debug!("Attempting to show error notification...");

                            let result = cx.update(|_, cx| {
                                log::debug!("Inside cx.update");
                                if let Some(window) = cx.active_window() {
                                    log::debug!("Found active window");
                                    window.update(cx, |_, window, cx| {
                                        log::debug!("Inside window.update, pushing notification");
                                        let error_note = Notification::error(format!(
                                            "Failed to generate comment: {}",
                                            e
                                        ))
                                        .id::<AiCommentError>();
                                        window.push_notification(error_note, cx);
                                        log::debug!("Notification pushed successfully");
                                    })
                                } else {
                                    log::warn!("No active window found!");
                                    Err(anyhow!("No active window"))
                                }
                            });

                            if let Err(e) = result {
                                log::error!("Failed to update context: {:?}", e);
                            }

                            Err(anyhow!("Failed to generate comment: {}", e))
                        }
                    }
                })
            }

            "explain" => {
                // Show loading notification
                struct AiExplainLoading;
                let loading_note =
                    Notification::info("Analyzing code with AI...").id::<AiExplainLoading>();
                window.push_notification(loading_note, cx);

                window.spawn(cx, async move |cx| {
                    // Call AI service
                    let explanation_result = ai_service.explain_code(&code).await;

                    match explanation_result {
                        Ok(explanation) => {
                            log::info!(
                                "=== Code Explanation ===\n{}\n========================",
                                explanation
                            );

                            // Show explanation in notification
                            cx.update(|_, cx| {
                                if let Some(window) = cx.active_window() {
                                    window.update(cx, |_, window, cx| {
                                        struct AiExplainResult;
                                        let success_note = Notification::success("Code explanation generated! Check logs for details.")
                                            .id::<AiExplainResult>();
                                        window.push_notification(success_note, cx);
                                    }).ok();
                                }
                            }).ok();

                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to explain code: {}", e);

                            // Show error notification
                            struct AiExplainError;
                            log::debug!("Attempting to show explain error notification...");

                            let result = cx.update(|_, cx| {
                                log::debug!("Inside cx.update for explain error");
                                if let Some(window) = cx.active_window() {
                                    log::debug!("Found active window for explain error");
                                    window.update(cx, |_, window, cx| {
                                        log::debug!("Inside window.update, pushing explain error notification");
                                        let error_note = Notification::error(format!("Failed to explain code: {}", e))
                                            .id::<AiExplainError>();
                                        window.push_notification(error_note, cx);
                                        log::debug!("Explain error notification pushed successfully");
                                    })
                                } else {
                                    log::warn!("No active window found for explain error!");
                                    Err(anyhow!("No active window"))
                                }
                            });

                            if let Err(e) = result {
                                log::error!("Failed to update context for explain error: {:?}", e);
                            }

                            Err(anyhow!("Failed to explain code: {}", e))
                        }
                    }
                })
            }

            "improve" => {
                // Show loading notification
                struct AiImproveLoading;
                let loading_note = Notification::info("Analyzing code for improvements with AI...")
                    .id::<AiImproveLoading>();
                window.push_notification(loading_note, cx);

                window.spawn(cx, async move |cx| {
                    // Call AI service
                    let suggestions_result = ai_service.suggest_improvements(&code).await;

                    match suggestions_result {
                        Ok(suggestions) => {
                            log::info!(
                                "=== Code Improvement Suggestions ===\n{}\n====================================",
                                suggestions
                            );

                            // Show success in notification
                            cx.update(|_, cx| {
                                if let Some(window) = cx.active_window() {
                                    window.update(cx, |_, window, cx| {
                                        struct AiImproveResult;
                                        let success_note = Notification::success("Code improvement suggestions generated! Check logs for details.")
                                            .id::<AiImproveResult>();
                                        window.push_notification(success_note, cx);
                                    }).ok();
                                }
                            }).ok();

                            Ok(())
                        }
                        Err(e) => {
                            log::error!("Failed to generate suggestions: {}", e);

                            // Show error notification
                            struct AiImproveError;
                            log::debug!("Attempting to show improve error notification...");

                            let result = cx.update(|_, cx| {
                                log::debug!("Inside cx.update for improve error");
                                if let Some(window) = cx.active_window() {
                                    log::debug!("Found active window for improve error");
                                    window.update(cx, |_, window, cx| {
                                        log::debug!("Inside window.update, pushing improve error notification");
                                        let error_note = Notification::error(format!("Failed to generate suggestions: {}", e))
                                            .id::<AiImproveError>();
                                        window.push_notification(error_note, cx);
                                        log::debug!("Improve error notification pushed successfully");
                                    })
                                } else {
                                    log::warn!("No active window found for improve error!");
                                    Err(anyhow!("No active window"))
                                }
                            });

                            if let Err(e) = result {
                                log::error!("Failed to update context for improve error: {:?}", e);
                            }

                            Err(anyhow!("Failed to generate suggestions: {}", e))
                        }
                    }
                })
            }

            _ => Task::ready(Err(anyhow!("Unknown AI action: {}", ai_action))),
        }
    }
}

/// Smart comment formatting based on code type and language
fn format_comment_for_code(
    code: &str,
    comment: &str,
    style: crate::core::services::CommentStyle,
) -> String {
    use crate::core::services::CommentStyle;

    let trimmed = code.trim();

    // Detect if this is a function or class definition
    let is_function = trimmed.starts_with("fn ")
        || trimmed.starts_with("pub fn ")
        || trimmed.starts_with("async fn ")
        || trimmed.contains("class ")
        || trimmed.contains("def ")
        || (trimmed.contains('(') && trimmed.contains('{'));

    match (is_function, style) {
        (true, CommentStyle::FunctionDoc) => {
            // Choose documentation comment format based on language
            if code.contains("fn ") || code.contains("impl ") {
                // Rust: /// style
                format!("/// {}\n{}", comment.replace('\n', "\n/// "), code)
            } else if code.starts_with("def ") {
                // Python: """docstring"""
                format!("\"\"\"\n{}\n\"\"\"\n{}", comment, code)
            } else if code.contains("function ") || code.contains("=>") {
                // JavaScript/TypeScript: /** JSDoc */
                format!("/**\n * {}\n */\n{}", comment.replace('\n', "\n * "), code)
            } else {
                // Default C-style
                format!("/*\n * {}\n */\n{}", comment.replace('\n', "\n * "), code)
            }
        }
        _ => {
            // Inline comment
            if code.contains("fn ") || code.contains("function ") {
                format!("// {}\n{}", comment, code)
            } else if code.starts_with("def ") || code.contains("import ") {
                format!("# {}\n{}", comment, code)
            } else {
                format!("// {}\n{}", comment, code)
            }
        }
    }
}
