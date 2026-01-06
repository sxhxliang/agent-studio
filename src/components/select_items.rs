use gpui::SharedString;
use gpui_component::select::SelectItem;

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
