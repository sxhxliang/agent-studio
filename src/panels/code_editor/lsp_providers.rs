use std::{ops::Range, str::FromStr, time::Duration};

use gpui::{App, AppContext, Context, Entity, Result, SharedString, Task, Window};
use gpui_component::input::{
    CodeActionProvider, CompletionProvider, DefinitionProvider, DocumentColorProvider,
    HoverProvider, InputState, Rope, RopeExt,
};
use lsp_types::{
    CodeAction, CodeActionKind, CompletionContext, CompletionResponse, TextEdit, WorkspaceEdit,
};

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

        actions.push(CodeAction {
            title: "Convert to Uppercase".into(),
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![TextEdit {
                            range,
                            new_text: old_text.to_uppercase(),
                            ..Default::default()
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

        actions.push(CodeAction {
            title: "Convert to Lowercase".into(),
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![TextEdit {
                            range: range,
                            new_text: old_text.to_lowercase(),
                            ..Default::default()
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

        actions.push(CodeAction {
            title: "Titleize".into(),
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![TextEdit {
                            range: range,
                            new_text: old_text
                                .split_whitespace()
                                .map(|word| {
                                    let mut chars = word.chars();
                                    chars
                                        .next()
                                        .map(|c| c.to_uppercase().collect::<String>())
                                        .unwrap_or_default()
                                        + chars.as_str()
                                })
                                .collect::<Vec<_>>()
                                .join(" "),
                            ..Default::default()
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

        actions.push(CodeAction {
            title: "Capitalize".into(),
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![TextEdit {
                            range,
                            new_text: old_text
                                .chars()
                                .enumerate()
                                .map(|(i, c)| {
                                    if i == 0 {
                                        c.to_uppercase().to_string()
                                    } else {
                                        c.to_string()
                                    }
                                })
                                .collect(),
                            ..Default::default()
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

        // snake_case
        actions.push(CodeAction {
            title: "Convert to snake_case".into(),
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(WorkspaceEdit {
                changes: Some(
                    std::iter::once((
                        document_uri.clone(),
                        vec![TextEdit {
                            range,
                            new_text: old_text
                                .chars()
                                .enumerate()
                                .map(|(i, c)| {
                                    if c.is_uppercase() {
                                        if i != 0 {
                                            format!("_{}", c.to_lowercase())
                                        } else {
                                            c.to_lowercase().to_string()
                                        }
                                    } else {
                                        c.to_string()
                                    }
                                })
                                .collect(),
                            ..Default::default()
                        }],
                    ))
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        });

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
