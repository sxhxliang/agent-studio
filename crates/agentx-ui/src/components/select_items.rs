//! Select-dropdown item types for the composer's agent / mode / model pickers.
//!
//! Pure data: each implements [`gpui_component::select::SelectItem`] so a
//! `SelectState<Vec<T>>` can render it. No services, no state — the parent owns
//! the `SelectState` and passes it to the composer.

use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window};
use gpui_component::{Icon, IconName, Sizable as _, h_flex, select::SelectItem};

/// An agent option, shown with a bot glyph in the dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentItem {
    pub name: String,
}

impl AgentItem {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl SelectItem for AgentItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.name.clone().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(IconName::Bot).xsmall())
                .child(self.name.clone())
                .into_any_element(),
        )
    }

    fn render(&self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .child(Icon::new(IconName::Bot).xsmall())
            .child(self.name.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }
}

/// A session-mode option (`ask` / `code` / `plan`, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeSelectItem {
    pub id: String,
    pub label: String,
}

impl ModeSelectItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

impl SelectItem for ModeSelectItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

/// A model option for the model picker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSelectItem {
    pub id: String,
    pub label: String,
}

impl ModelSelectItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

impl SelectItem for ModelSelectItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}
