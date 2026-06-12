//! Visual preview gallery for the AgentX UI.
//!
//! Boots a real GPUI window (no agents, no filesystem) and renders the reusable
//! components and panels with mock data, so their look can be checked against the
//! legacy `src/panels` / `src/components` originals without running an agent.
//!
//! ```text
//! cargo run -p agentx-ui --example gallery
//! ```
//!
//! Sections are added as each migration slice lands. Today: the component
//! primitives (status tones + the composer's select dropdowns).

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Root, Sizable as _, h_flex,
    input::{Input, InputState},
    select::{Select, SelectState},
    v_flex,
};

use agentx_domain::{
    AgentConfig, CommandConfig, Config, McpServerConfig, ModelConfig, SessionStatus, SlashCommand,
};
use agentx_ui::components::{
    AgentItem, ChatInputBox, CodeSelection, FileItem, ImageAttachment, InputSuggestion,
    InputSuggestionState, ModeSelectItem, ModelSelectItem, render_file_item, session_status_tone,
    status_badge, status_dot,
};
use agentx_ui::panels::{SettingsPanel, TaskPanel};

/// Every status a session can be in, for the status-tone swatch row.
const ALL_STATUSES: [SessionStatus; 6] = [
    SessionStatus::Pending,
    SessionStatus::Running,
    SessionStatus::Idle,
    SessionStatus::Completed,
    SessionStatus::Failed,
    SessionStatus::Closed,
];

struct Gallery {
    agent_select: Entity<SelectState<Vec<AgentItem>>>,
    mode_select: Entity<SelectState<Vec<ModeSelectItem>>>,
    model_select: Entity<SelectState<Vec<ModelSelectItem>>>,
    commands: Entity<InputSuggestionState<SharedString>>,
    composer_input: Entity<InputState>,
    composer_busy_input: Entity<InputState>,
    c_agent_select: Entity<SelectState<Vec<AgentItem>>>,
    c_mode_select: Entity<SelectState<Vec<ModeSelectItem>>>,
    c_model_select: Entity<SelectState<Vec<ModelSelectItem>>>,
    settings: Entity<SettingsPanel>,
    tasks: Entity<TaskPanel>,
    scroll: ScrollHandle,
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let agent_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    AgentItem::new("Claude"),
                    AgentItem::new("Codex"),
                    AgentItem::new("Gemini"),
                ],
                None,
                window,
                cx,
            )
        });
        let mode_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    ModeSelectItem::new("ask", "Ask"),
                    ModeSelectItem::new("code", "Code"),
                    ModeSelectItem::new("plan", "Plan"),
                ],
                None,
                window,
                cx,
            )
        });
        let model_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    ModelSelectItem::new("sonnet", "Claude Sonnet"),
                    ModelSelectItem::new("opus", "Claude Opus"),
                ],
                None,
                window,
                cx,
            )
        });
        let commands = cx.new(|cx| {
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Type / for commands…"));
            InputSuggestionState::with_input(input, window, cx)
        });
        let composer_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(2, 8)
                .placeholder("Type a message, press Enter to send…")
                .default_value("Refactor the auth module to use the new token store")
        });
        let composer_busy_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(2, 8)
                .placeholder("Working…")
        });
        let c_agent_select = cx.new(|cx| {
            SelectState::new(
                vec![AgentItem::new("Claude"), AgentItem::new("Codex")],
                None,
                window,
                cx,
            )
        });
        let c_mode_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    ModeSelectItem::new("ask", "Ask"),
                    ModeSelectItem::new("code", "Code"),
                ],
                None,
                window,
                cx,
            )
        });
        let c_model_select = cx.new(|cx| {
            SelectState::new(
                vec![
                    ModelSelectItem::new("sonnet", "Claude Sonnet"),
                    ModelSelectItem::new("opus", "Claude Opus"),
                ],
                None,
                window,
                cx,
            )
        });
        let settings = cx.new(|cx| SettingsPanel::new(mock_config(), None, cx));
        let tasks = cx.new(|cx| TaskPanel::preview(window, cx));
        Self {
            agent_select,
            mode_select,
            model_select,
            commands,
            composer_input,
            composer_busy_input,
            c_agent_select,
            c_mode_select,
            c_model_select,
            settings,
            tasks,
            scroll: ScrollHandle::new(),
        }
    }
}

/// A populated config so every settings page has entries to show.
fn mock_config() -> Config {
    let mut config = Config::default();
    config.proxy.enabled = true;
    config.proxy.http_proxy_url = "http://127.0.0.1:7890".into();
    config.agents.insert(
        "claude".into(),
        AgentConfig {
            command: "claude-code-acp".into(),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
        },
    );
    config.agents.insert(
        "codex".into(),
        AgentConfig {
            command: "codex-acp".into(),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
        },
    );
    config.models.insert(
        "gpt-4o".into(),
        ModelConfig {
            enabled: true,
            provider: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model_name: "gpt-4o".into(),
        },
    );
    config
        .mcp_servers
        .insert("filesystem".into(), mock_mcp("filesystem", true));
    config
        .mcp_servers
        .insert("postgres".into(), mock_mcp("postgres", false));
    config.commands.insert(
        "review".into(),
        CommandConfig {
            description: "Review the diff".into(),
            template: "Review the current diff".into(),
        },
    );
    config
        .system_prompts
        .insert("explain".into(), "Explain the selected code".into());
    config
}

/// A mock MCP server config for the composer demo.
fn mock_mcp(name: &str, enabled: bool) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        enabled,
        command: "npx".to_string(),
        args: Vec::new(),
        env: std::collections::HashMap::new(),
    }
}

/// A labelled section: a bold title over its body.
fn section(title: &str, foreground: Hsla, body: impl IntoElement) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(foreground)
                .child(title.to_string()),
        )
        .child(body)
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let border = theme.border;
        let background = theme.background;

        let status_row =
            h_flex()
                .gap_3()
                .flex_wrap()
                .children(ALL_STATUSES.into_iter().map(|status| {
                    status_badge(format!("{status:?}"), session_status_tone(status), theme)
                }));

        let dot_row = h_flex().gap_3().items_center().children(
            ALL_STATUSES
                .into_iter()
                .map(|status| status_dot(session_status_tone(status), theme)),
        );

        let selects = h_flex()
            .gap_4()
            .items_center()
            .child(Select::new(&self.agent_select).small())
            .child(Select::new(&self.mode_select).small())
            .child(Select::new(&self.model_select).small());

        let suggestion = v_flex()
            .w(px(440.))
            .gap_2()
            .p_3()
            .rounded(px(12.))
            .border_1()
            .border_color(border)
            .bg(background)
            .shadow_md()
            .child(
                InputSuggestion::new(&self.commands)
                    .id("gallery-commands")
                    .header("Available Commands")
                    .max_height(px(200.))
                    .items(vec![
                        SharedString::from("/help"),
                        SharedString::from("/clear"),
                        SharedString::from("/model"),
                        SharedString::from("/compact"),
                    ])
                    .input(|state| Input::new(state).appearance(false)),
            );

        let files = v_flex()
            .w(px(440.))
            .gap_1()
            .p_2()
            .rounded(px(8.))
            .border_1()
            .border_color(border)
            .bg(background)
            .children([
                render_file_item(&FileItem::new("main.rs", "src/main.rs", false), theme),
                render_file_item(&FileItem::new("lib.rs", "src/lib.rs", false), theme),
                render_file_item(&FileItem::new("components", "src/components", true), theme),
                render_file_item(&FileItem::new("Cargo.toml", "Cargo.toml", false), theme),
            ]);

        let composer_idle = ChatInputBox::new("composer-idle", self.composer_input.clone())
            .title("New task")
            .agent_select(self.c_agent_select.clone())
            .agent_status_text("ready")
            .mode_select(self.c_mode_select.clone())
            .model_select(self.c_model_select.clone())
            .command_suggestions(vec![
                SlashCommand {
                    name: "help".into(),
                    description: "Show help".into(),
                },
                SlashCommand {
                    name: "clear".into(),
                    description: "Clear the conversation".into(),
                },
            ])
            .available_mcps(vec![
                ("filesystem".into(), mock_mcp("filesystem", true)),
                ("git".into(), mock_mcp("git", true)),
                ("postgres".into(), mock_mcp("postgres", false)),
            ])
            .selected_mcps(vec!["filesystem".into()])
            .session_status(Some(SessionStatus::Idle));

        let composer_busy = ChatInputBox::new("composer-busy", self.composer_busy_input.clone())
            .pasted_images(vec![ImageAttachment {
                filename: "screenshot.png".into(),
                mime_type: "image/png".into(),
                data: String::new(),
            }])
            .code_selections(vec![CodeSelection {
                file_path: "src/auth/token.rs".into(),
                start_line: 42,
                end_line: 58,
            }])
            .selected_files(vec!["src/main.rs".into()])
            .session_status(Some(SessionStatus::Running));

        div()
            .id("gallery-scroll")
            .size_full()
            .track_scroll(&self.scroll)
            .overflow_y_scroll()
            .child(
                v_flex()
                    .p_6()
                    .gap_6()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .child("AgentX UI Gallery"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child("Mock-data preview of reusable components and panels."),
                    )
                    .child(section("Status badges", foreground, status_row))
                    .child(section("Status dots", foreground, dot_row))
                    .child(section(
                        "Select items (agent / mode / model)",
                        foreground,
                        selects,
                    ))
                    .child(section(
                        "Input suggestion (click in, type, ↑/↓/Enter)",
                        foreground,
                        suggestion,
                    ))
                    .child(section("File suggestion rows", foreground, files))
                    .child(section(
                        "Composer — idle (send enabled, selects + MCP + /commands)",
                        foreground,
                        composer_idle,
                    ))
                    .child(section(
                        "Composer — running (attachments + red cancel)",
                        foreground,
                        composer_busy,
                    ))
                    .child(section(
                        "Settings panel (9 pages, mock config)",
                        foreground,
                        div()
                            .h(px(520.))
                            .w_full()
                            .border_1()
                            .border_color(border)
                            .rounded(px(8.))
                            .overflow_hidden()
                            .child(self.settings.clone()),
                    ))
                    .child(section(
                        "Task panel (Tree / Timeline, mock workspaces)",
                        foreground,
                        div()
                            .h(px(420.))
                            .w(px(360.))
                            .border_1()
                            .border_color(border)
                            .rounded(px(8.))
                            .overflow_hidden()
                            .child(self.tasks.clone()),
                    )),
            )
    }
}

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);
    app.run(|cx| {
        gpui_component::init(cx);
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(1100.0), px(820.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };

        let _ = cx.open_window(options, |window, cx| {
            let gallery = cx.new(|cx| Gallery::new(window, cx));
            cx.new(|cx| Root::new(gallery, window, cx).bg(cx.theme().background))
        });
    });
}
