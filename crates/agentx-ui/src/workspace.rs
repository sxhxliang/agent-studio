//! The dock shell — hosts panels in a gpui-component `DockArea`.
//!
//! Layout: the session list in the left dock, the chat in the center. As more
//! panels are built (settings, tasks, a code editor, …) they get added here;
//! this is the seam that grows toward replacing the legacy workspace.

use gpui::*;
use gpui_component::dock::{DockArea, DockItem};

use crate::chat::{ChatView, SessionsPanel};

/// The window's root content: a `DockArea` with the sessions panel docked left
/// and the chat panel in the center.
pub struct Workspace {
    dock_area: Entity<DockArea>,
}

impl Workspace {
    pub fn new(
        chat: Entity<ChatView>,
        sessions: Entity<SessionsPanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let dock_area = cx.new(|cx| DockArea::new("agentx-main", Some(1), window, cx));
        let weak = dock_area.downgrade();
        let center = DockItem::tab(chat, &weak, window, cx);
        let left = DockItem::tab(sessions, &weak, window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(center, window, cx);
            area.set_left_dock(left, Some(px(240.)), true, window, cx);
        });
        Self { dock_area }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.dock_area.clone()
    }
}
