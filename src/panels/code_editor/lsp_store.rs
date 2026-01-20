use std::sync::{Arc, RwLock};

use gpui_component::highlighter::Diagnostic;
use lsp_types::{CodeAction, CompletionItem};
use std::ops::Range;

#[derive(Clone)]
pub struct CodeEditorPanelLspStore {
    pub(super) completions: Arc<Vec<CompletionItem>>,
    pub(super) code_actions: Arc<RwLock<Vec<(Range<usize>, CodeAction)>>>,
    pub(super) diagnostics: Arc<RwLock<Vec<Diagnostic>>>,
    pub(super) dirty: Arc<RwLock<bool>>,
}

impl CodeEditorPanelLspStore {
    pub fn new() -> Self {
        // let completions = serde_json::from_slice::<Vec<CompletionItem>>(include_bytes!(
        //     "../../fixtures/completion_items.json"
        // ))
        // .unwrap();

        Self {
            completions: Arc::new(vec![]),
            code_actions: Arc::new(RwLock::new(vec![])),
            diagnostics: Arc::new(RwLock::new(vec![])),
            dirty: Arc::new(RwLock::new(false)),
        }
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let guard = self.diagnostics.read().unwrap();
        guard.clone()
    }

    pub fn update_diagnostics(&self, diagnostics: Vec<Diagnostic>) {
        let mut guard = self.diagnostics.write().unwrap();
        *guard = diagnostics;
        *self.dirty.write().unwrap() = true;
    }

    pub fn code_actions(&self) -> Vec<(Range<usize>, CodeAction)> {
        let guard = self.code_actions.read().unwrap();
        guard.clone()
    }

    pub fn update_code_actions(&self, code_actions: Vec<(Range<usize>, CodeAction)>) {
        let mut guard = self.code_actions.write().unwrap();
        *guard = code_actions;
        *self.dirty.write().unwrap() = true;
    }

    pub fn is_dirty(&self) -> bool {
        let guard = self.dirty.read().unwrap();
        *guard
    }
}
