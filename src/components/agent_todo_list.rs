use gpui::{
    div, px, App, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, Styled, Window,
};

use gpui_component::{h_flex, v_flex, ActiveTheme, Icon, IconName};

/// Plan entry priority levels
#[derive(Clone, Debug, PartialEq)]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
}

impl Default for PlanEntryPriority {
    fn default() -> Self {
        Self::Medium
    }
}

impl PlanEntryPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

/// Plan entry status
#[derive(Clone, Debug, PartialEq)]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
}

impl Default for PlanEntryStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl PlanEntryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        }
    }
}

/// A single plan entry representing a task or goal
#[derive(Clone, Debug)]
pub struct PlanEntry {
    /// A human-readable description of what this task aims to accomplish
    pub content: SharedString,
    /// The relative importance of this task
    pub priority: PlanEntryPriority,
    /// The current execution status of this task
    pub status: PlanEntryStatus,
}

impl PlanEntry {
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            priority: PlanEntryPriority::default(),
            status: PlanEntryStatus::default(),
        }
    }

    pub fn with_priority(mut self, priority: PlanEntryPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_status(mut self, status: PlanEntryStatus) -> Self {
        self.status = status;
        self
    }
}

/// A list item component for displaying a plan entry
#[derive(IntoElement)]
struct PlanEntryItem {
    id: ElementId,
    entry: PlanEntry,
}

impl PlanEntryItem {
    pub fn new(id: impl Into<ElementId>, entry: PlanEntry) -> Self {
        Self {
            id: id.into(),
            entry,
        }
    }
}

impl RenderOnce for PlanEntryItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = match self.entry.status {
            PlanEntryStatus::Completed => cx.theme().muted_foreground,
            _ => cx.theme().foreground,
        };

        // Select icon and color based on status
        let (icon, icon_color) = match self.entry.status {
            PlanEntryStatus::Completed => (IconName::CircleCheck, cx.theme().green),
            PlanEntryStatus::InProgress => (IconName::LoaderCircle, cx.theme().accent),
            PlanEntryStatus::Pending => (IconName::Dash, cx.theme().muted_foreground),
        };

        div().id(self.id).child(
            h_flex()
                .items_start()
                .gap_2()
                .child(
                    div()
                        .mt(px(1.))
                        .child(Icon::new(icon).text_color(icon_color).size(px(16.))),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(14.))
                        .text_color(text_color)
                        .line_height(px(20.))
                        .child(self.entry.content),
                ),
        )
    }
}

/// Agent Todo List component for displaying plan execution progress
pub struct AgentTodoList {
    entries: Vec<PlanEntry>,
    title: SharedString,
}

impl AgentTodoList {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            title: "Tasks".into(),
        }
    }

    /// Set the title of the todo list
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the plan entries
    pub fn entries(mut self, entries: Vec<PlanEntry>) -> Self {
        self.entries = entries;
        self
    }

    /// Add a single entry
    pub fn entry(mut self, entry: PlanEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Get the count of completed tasks
    fn completed_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.status == PlanEntryStatus::Completed)
            .count()
    }

    /// Get the total count of tasks
    fn total_count(&self) -> usize {
        self.entries.len()
    }
}

impl Default for AgentTodoList {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoElement for AgentTodoList {
    type Element = gpui::Div;

    fn into_element(self) -> Self::Element {
        let title = self.title.clone();
        let completed = self.completed_count();
        let total = self.total_count();

        v_flex()
            .gap_3()
            .w_full()
            .child(
                // Header with title and count
                h_flex()
                    .justify_between()
                    .items_center()
                    .w_full()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(IconName::LayoutDashboard).size(px(16.)))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .child(format!("{}/{}", completed, total)),
                    ),
            )
            .child(
                // Task list
                v_flex()
                    .gap_2()
                    .w_full()
                    .children(self.entries.into_iter().enumerate().map(|(i, entry)| {
                        PlanEntryItem::new(SharedString::from(format!("plan-entry-{}", i)), entry)
                    })),
            )
    }
}

/// A stateful wrapper around AgentTodoList that can be used as a GPUI view
pub struct AgentTodoListView {
    entries: Entity<Vec<PlanEntry>>,
    title: SharedString,
}

impl AgentTodoListView {
    pub fn new(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let entries = cx.new(|_| Vec::new());
            Self {
                entries,
                title: "Tasks".into(),
            }
        })
    }

    /// Create a new view with entries
    pub fn with_entries(
        entries: Vec<PlanEntry>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let entries_entity = cx.new(|_| entries);
            Self {
                entries: entries_entity,
                title: "Tasks".into(),
            }
        })
    }

    /// Update the entries
    pub fn set_entries(&mut self, entries: Vec<PlanEntry>, cx: &mut App) {
        self.entries.update(cx, |e, cx| {
            *e = entries;
            cx.notify();
        });
    }

    /// Add a new entry
    pub fn add_entry(&mut self, entry: PlanEntry, cx: &mut App) {
        self.entries.update(cx, |e, cx| {
            e.push(entry);
            cx.notify();
        });
    }

    /// Update an entry at a specific index
    pub fn update_entry(&mut self, index: usize, entry: PlanEntry, cx: &mut App) {
        self.entries.update(cx, |e, cx| {
            if let Some(existing) = e.get_mut(index) {
                *existing = entry;
                cx.notify();
            }
        });
    }

    /// Update the status of an entry at a specific index
    pub fn update_status(&mut self, index: usize, status: PlanEntryStatus, cx: &mut App) {
        self.entries.update(cx, |e, cx| {
            if let Some(entry) = e.get_mut(index) {
                entry.status = status;
                cx.notify();
            }
        });
    }

    /// Set the title
    pub fn set_title(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.title = title.into();
        cx.notify();
    }
}

impl Render for AgentTodoListView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self.entries.read(cx).clone();

        AgentTodoList::new()
            .title(self.title.clone())
            .entries(entries)
    }
}
