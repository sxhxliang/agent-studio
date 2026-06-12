//! `TaskPanel` rendering — a Tree of workspace groups or a Timeline of tasks.
//!
//! Reads state and builds elements; click handlers call back into the intents
//! defined on [`TaskPanel`] in the parent module.

use chrono::{DateTime, Utc};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelEvent},
    h_flex,
    input::Input,
    v_flex,
};

use agentx_domain::{SessionId, SessionStatus, TaskId, WorkspaceId};

use crate::components::{StatusTone, session_status_tone, status_badge};

use super::{TaskPanel, ViewMode};

/// A flattened, owned snapshot of one task row (so the build borrows neither
/// `self` nor the theme while wiring click handlers).
struct RowData {
    task: TaskId,
    name: String,
    agent: String,
    status: SessionStatus,
    session: Option<SessionId>,
    time: String,
    bucket: &'static str,
    last_message: Option<String>,
}

struct GroupData {
    id: WorkspaceId,
    name: String,
    expanded: bool,
    rows: Vec<RowData>,
}

fn relative_time(created: DateTime<Utc>) -> String {
    let minutes = Utc::now()
        .signed_duration_since(created)
        .num_minutes()
        .max(0);
    if minutes < 1 {
        "just now".to_string()
    } else if minutes < 60 {
        format!("{minutes}m ago")
    } else if minutes < 60 * 24 {
        format!("{}h ago", minutes / 60)
    } else {
        format!("{}d ago", minutes / (60 * 24))
    }
}

/// Bucket a task by age, for the Timeline view.
fn time_bucket(created: DateTime<Utc>) -> &'static str {
    let days = Utc::now().signed_duration_since(created).num_days();
    if days < 1 {
        "Today"
    } else if days < 2 {
        "Yesterday"
    } else {
        "Older"
    }
}

impl TaskPanel {
    fn snapshot(&self, cx: &Context<Self>) -> Vec<GroupData> {
        let query = self.search.read(cx).value().to_lowercase();
        self.groups
            .iter()
            .map(|group| GroupData {
                id: group.id.clone(),
                name: group.name.clone(),
                expanded: group.expanded,
                rows: group
                    .tasks
                    .iter()
                    .filter(|task| query.is_empty() || task.name.to_lowercase().contains(&query))
                    .map(|task| RowData {
                        task: task.id.clone(),
                        name: task.name.clone(),
                        agent: task.agent.to_string(),
                        status: task.status,
                        session: task.session.clone(),
                        time: relative_time(task.created_at),
                        bucket: time_bucket(task.created_at),
                        last_message: task.last_message.clone(),
                    })
                    .collect(),
            })
            .collect()
    }
}

impl Render for TaskPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let background = theme.background;
        let border = theme.border;
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let accent = theme.accent;
        let primary = theme.primary;
        let success = theme.success;
        let warning = theme.warning;
        let danger = theme.danger;
        let selected_bg = theme.accent;

        let dot = move |status: SessionStatus| {
            let color = match session_status_tone(status) {
                StatusTone::Neutral => muted,
                StatusTone::Info => primary,
                StatusTone::Success => success,
                StatusTone::Warning => warning,
                StatusTone::Danger => danger,
            };
            div().size(px(8.)).rounded_full().bg(color)
        };

        let view_mode = self.view_mode;
        let selected = self.selected.clone();
        let snapshot = self.snapshot(cx);

        // Header: search + Tree/Timeline toggle.
        let header = h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&self.search).small().cleanable(true)),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("view-tree")
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(IconName::LayoutDashboard))
                            .selected(view_mode == ViewMode::Tree)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.set_view_mode(ViewMode::Tree, cx);
                            })),
                    )
                    .child(
                        Button::new("view-timeline")
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(IconName::Menu))
                            .selected(view_mode == ViewMode::Timeline)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.set_view_mode(ViewMode::Timeline, cx);
                            })),
                    ),
            );

        // A single task row (two lines), shared by both views.
        let task_row = |row: &RowData, indent: bool| {
            let is_selected = selected.as_ref() == Some(&row.task);
            let row_bg = if is_selected { selected_bg } else { background };
            let open_session = row.session.clone();
            let select_id = row.task.clone();
            let subtitle = match &row.last_message {
                Some(message) => format!("{} · {}", row.agent, message),
                None => row.agent.clone(),
            };
            v_flex()
                .id(SharedString::from(format!("task-{}", row.task)))
                .w_full()
                .gap_0p5()
                .px_3()
                .when(indent, |this| this.pl(px(34.)))
                .py_2()
                .rounded(px(6.))
                .bg(row_bg)
                .hover(move |this| this.bg(accent.opacity(0.4)))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.select_task(select_id.clone(), cx);
                    if let Some(session) = open_session.clone() {
                        this.open_task(session, cx);
                    }
                }))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(dot(row.status))
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(foreground)
                                .overflow_x_hidden()
                                .text_ellipsis()
                                .child(row.name.clone()),
                        )
                        .child(div().text_xs().text_color(muted).child(row.time.clone())),
                )
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(muted)
                                .overflow_x_hidden()
                                .text_ellipsis()
                                .child(subtitle),
                        )
                        .child(status_badge(
                            format!("{:?}", row.status),
                            session_status_tone(row.status),
                            theme,
                        )),
                )
                .into_any_element()
        };

        let body = match view_mode {
            ViewMode::Tree => {
                let mut items: Vec<AnyElement> = Vec::new();
                for group in &snapshot {
                    let group_id = group.id.clone();
                    let count = group.rows.len();
                    let chevron = if group.expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    };
                    items.push(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_1p5()
                            .child(
                                Button::new(SharedString::from(format!("ws-{}", group.id)))
                                    .ghost()
                                    .xsmall()
                                    .icon(Icon::new(chevron))
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.toggle_workspace(group_id.clone(), cx);
                                    })),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(foreground)
                                    .child(group.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("({count})")),
                            )
                            .into_any_element(),
                    );
                    if group.expanded {
                        for row in &group.rows {
                            items.push(task_row(row, true));
                        }
                    }
                }
                v_flex().w_full().gap_0p5().children(items)
            }
            ViewMode::Timeline => {
                let all: Vec<&RowData> = snapshot.iter().flat_map(|g| g.rows.iter()).collect();
                let mut items: Vec<AnyElement> = Vec::new();
                for bucket in ["Today", "Yesterday", "Older"] {
                    let bucket_rows: Vec<&&RowData> =
                        all.iter().filter(|row| row.bucket == bucket).collect();
                    if bucket_rows.is_empty() {
                        continue;
                    }
                    items.push(
                        div()
                            .px_3()
                            .py_1()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(muted)
                            .child(bucket.to_uppercase())
                            .into_any_element(),
                    );
                    for row in bucket_rows {
                        items.push(task_row(row, false));
                    }
                }
                v_flex().w_full().gap_0p5().children(items)
            }
        };

        v_flex()
            .size_full()
            .bg(background)
            .child(header)
            .child(
                div()
                    .id("task-list")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .child(v_flex().w_full().p_1().child(body)),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .py_1p5()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        Button::new("tasks-refresh")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::LoaderCircle))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.refresh(cx);
                            })),
                    ),
            )
    }
}

impl EventEmitter<PanelEvent> for TaskPanel {}

impl Focusable for TaskPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for TaskPanel {
    fn panel_name(&self) -> &'static str {
        "TaskPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Tasks"
    }
}
