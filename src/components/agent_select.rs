use gpui::{AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window};
use gpui_component::{Icon, Sizable, h_flex, select::SelectItem};

/// An agent item with icon for the select dropdown
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
        let icon = crate::assets::get_agent_icon(&self.name);
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .child(Icon::new(icon).xsmall())
                .child(self.name.clone())
                .into_any_element(),
        )
    }

    fn render(&self, _window: &mut Window, _cx: &mut App) -> impl gpui::IntoElement {
        let icon = crate::assets::get_agent_icon(&self.name);
        h_flex()
            .gap_2()
            .items_center()
            .child(Icon::new(icon).xsmall())
            .child(self.name.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }
}
