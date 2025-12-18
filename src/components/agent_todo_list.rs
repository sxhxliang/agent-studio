use gpui::{
    App, AppContext, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, RenderOnce, SharedString, Styled, Window, div, px,
};

use agent_client_protocol::{Plan, PlanEntry, PlanEntryPriority, PlanEntryStatus};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex, v_flex};
use serde::{Deserialize, Serialize};

/// Extended metadata for Plan (stored in Plan's meta field)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMeta {
    /// Optional title for the plan
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
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
            // Handle future variants
            _ => (IconName::Dash, cx.theme().muted_foreground),
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
                        .child(self.entry.content.clone()),
                ),
        )
    }
}

/// Agent Todo List component for displaying plan execution progress
/// Based on ACP's Plan structure from SessionUpdate::Plan
pub struct AgentTodoList {
    /// The plan data following ACP's Plan structure
    plan: Plan,
    /// Extended metadata (title, etc.) - extracted from plan.meta
    meta: PlanMeta,
}

impl AgentTodoList {
    pub fn new() -> Self {
        Self {
            plan: Plan::new(Vec::new()),
            meta: PlanMeta::default(),
        }
    }

    /// Create from an ACP Plan
    pub fn from_plan(plan: Plan) -> Self {
        // Extract title from meta if present
        let meta = plan
            .meta
            .as_ref()
            .and_then(|m| {
                serde_json::from_value::<PlanMeta>(serde_json::Value::Object(m.clone())).ok()
            })
            .unwrap_or_default();

        Self { plan, meta }
    }

    /// Set the title of the todo list (stored in meta)
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.meta.title = Some(title.into());
        self
    }

    /// Set the plan entries
    pub fn entries(mut self, entries: Vec<PlanEntry>) -> Self {
        self.plan.entries = entries;
        self
    }

    /// Add a single entry
    pub fn entry(mut self, entry: PlanEntry) -> Self {
        self.plan.entries.push(entry);
        self
    }

    /// Add a simple entry with content, priority, and status
    pub fn add_entry(
        mut self,
        content: impl Into<String>,
        priority: PlanEntryPriority,
        status: PlanEntryStatus,
    ) -> Self {
        self.plan
            .entries
            .push(PlanEntry::new(content, priority, status));
        self
    }

    /// Get the underlying Plan
    pub fn into_plan(mut self) -> Plan {
        // Store meta back into plan
        if self.meta.title.is_some() {
            self.plan.meta = serde_json::to_value(&self.meta)
                .ok()
                .and_then(|v| v.as_object().cloned());
        }
        self.plan
    }

    /// Get the count of completed tasks
    fn completed_count(&self) -> usize {
        self.plan
            .entries
            .iter()
            .filter(|e| e.status == PlanEntryStatus::Completed)
            .count()
    }

    /// Get the total count of tasks
    fn total_count(&self) -> usize {
        self.plan.entries.len()
    }

    /// Get the display title
    fn display_title(&self) -> &str {
        self.meta.title.as_deref().unwrap_or("Tasks")
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
        let title = self.display_title().to_string();
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
                    .children(self.plan.entries.into_iter().enumerate().map(|(i, entry)| {
                        PlanEntryItem::new(SharedString::from(format!("plan-entry-{}", i)), entry)
                    })),
            )
    }
}

/// A stateful wrapper around AgentTodoList that can be used as a GPUI view
pub struct AgentTodoListView {
    plan: Entity<Plan>,
    meta: PlanMeta,
}

impl AgentTodoListView {
    pub fn new(_window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let plan = cx.new(|_| Plan::new(Vec::new()));
            Self {
                plan,
                meta: PlanMeta::default(),
            }
        })
    }

    /// Create a new view with a Plan
    pub fn with_plan(plan: Plan, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        // Extract title from meta if present
        let meta = plan
            .meta
            .as_ref()
            .and_then(|m| {
                serde_json::from_value::<PlanMeta>(serde_json::Value::Object(m.clone())).ok()
            })
            .unwrap_or_default();

        cx.new(|cx| {
            let plan_entity = cx.new(|_| plan);
            Self {
                plan: plan_entity,
                meta,
            }
        })
    }

    /// Create a new view with entries (convenience method)
    pub fn with_entries(
        entries: Vec<PlanEntry>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let plan_entity = cx.new(|_| Plan::new(entries));
            Self {
                plan: plan_entity,
                meta: PlanMeta::default(),
            }
        })
    }

    /// Update the entire plan
    pub fn set_plan(&mut self, plan: Plan, cx: &mut App) {
        // Extract title from meta if present
        self.meta = plan
            .meta
            .as_ref()
            .and_then(|m| {
                serde_json::from_value::<PlanMeta>(serde_json::Value::Object(m.clone())).ok()
            })
            .unwrap_or_default();

        self.plan.update(cx, |p, cx| {
            *p = plan;
            cx.notify();
        });
    }

    /// Update the entries
    pub fn set_entries(&mut self, entries: Vec<PlanEntry>, cx: &mut App) {
        self.plan.update(cx, |p, cx| {
            p.entries = entries;
            cx.notify();
        });
    }

    /// Add a new entry
    pub fn add_entry(&mut self, entry: PlanEntry, cx: &mut App) {
        self.plan.update(cx, |p, cx| {
            p.entries.push(entry);
            cx.notify();
        });
    }

    /// Update an entry at a specific index
    pub fn update_entry(&mut self, index: usize, entry: PlanEntry, cx: &mut App) {
        self.plan.update(cx, |p, cx| {
            if let Some(existing) = p.entries.get_mut(index) {
                *existing = entry;
                cx.notify();
            }
        });
    }

    /// Update the status of an entry at a specific index
    pub fn update_status(&mut self, index: usize, status: PlanEntryStatus, cx: &mut App) {
        self.plan.update(cx, |p, cx| {
            if let Some(entry) = p.entries.get_mut(index) {
                entry.status = status.clone();
                cx.notify();
            }
        });
    }

    /// Set the title
    pub fn set_title(&mut self, title: impl Into<String>, cx: &mut Context<Self>) {
        self.meta.title = Some(title.into());
        cx.notify();
    }
}

impl Render for AgentTodoListView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let plan = self.plan.read(cx).clone();

        let mut todo_list = AgentTodoList::from_plan(plan);
        // Override with local meta if set
        if self.meta.title.is_some() {
            todo_list.meta = self.meta.clone();
        }
        todo_list
    }
}
