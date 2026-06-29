//! Rendering for the dock-hosted welcome panel.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Root, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelEvent},
    h_flex, v_flex,
};

use agentx_domain::SlashCommand;

use crate::components::ChatInputBox;
use crate::components::FileItem;

use super::WelcomePanel;

impl Render for WelcomePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.config_reload_pending {
            self.reload_config(window, cx);
        }

        let theme = cx.theme();
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let primary = theme.primary;
        let danger = theme.danger;
        let background = theme.background;
        let workspace_label = self
            .workspace_name()
            .map(|name| format!("Current workspace: {name}"))
            .unwrap_or_else(|| "Preparing workspace...".to_string());

        let entity = cx.entity().clone();
        let mut composer = ChatInputBox::new("welcome-chat-input", self.input.clone())
            .agent_select(self.agent_select.clone())
            .agent_status_text(self.session_status_text())
            .pasted_images(self.pasted_images.clone())
            .code_selections(self.code_selections.clone())
            .selected_files(self.selected_files.clone())
            .file_suggestions(self.file_suggestions.clone())
            .command_suggestions(self.command_suggestions.clone())
            .show_command_suggestions(self.show_command_suggestions)
            .available_mcps(self.available_mcps.clone())
            .selected_mcps(self.selected_mcps.clone())
            .disabled(self.selected_agent.is_none() || self.session_loading)
            .on_file_select(cx.listener(|this, file: &FileItem, window, cx| {
                this.apply_file_mention(file.clone(), window, cx);
            }))
            .on_command_select(cx.listener(|this, command: &SlashCommand, window, cx| {
                this.apply_command_selection(command.clone(), window, cx);
            }))
            .on_mcp_toggle(
                cx.listener(|this, (name, checked): &(String, bool), window, cx| {
                    if *checked {
                        if !this.selected_mcps.contains(name) {
                            this.selected_mcps.push(name.clone());
                        }
                    } else {
                        this.selected_mcps.retain(|selected| selected != name);
                    }
                    this.selected_mcps.sort();
                    this.mcp_selection_overridden = true;
                    this.on_mcp_selection_changed(window, cx);
                    cx.notify();
                }),
            )
            .on_paste(move |_window, cx| {
                entity.update(cx, |this, cx| this.handle_paste(cx));
            })
            .on_remove_image(cx.listener(|this, index, _window, cx| {
                this.remove_image(*index, cx);
            }))
            .on_remove_code_selection(cx.listener(|this, index, _window, cx| {
                this.remove_code_selection(*index, cx);
            }))
            .on_remove_file(cx.listener(|this, index, _window, cx| {
                this.remove_file(*index, cx);
            }))
            .on_send(cx.listener(|this, _event, window, cx| {
                this.handle_send(window, cx);
            }));

        if self.current_session_id.is_some() {
            if self.has_modes(cx) {
                composer = composer.mode_select(self.mode_select.clone());
            }
            if self.has_models(cx) {
                composer = composer.model_select(self.model_select.clone());
            }
        }

        let settings_service = self.config_service.clone();
        let status = self.status.clone();

        div()
            .size_full()
            .relative()
            .track_focus(&self.focus_handle)
            .bg(background)
            .child(
                h_flex().absolute().top_2().right_2().child(
                    Button::new("welcome-settings")
                        .ghost()
                        .small()
                        .icon(Icon::new(IconName::Settings))
                        .on_click(move |_event, _window, cx| {
                            crate::panels::open_settings_window(settings_service.clone(), cx);
                        }),
                ),
            )
            .child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .p_8()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(800.))
                            .gap_4()
                            .child(
                                v_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_3()
                                    .px_8()
                                    .pb_2()
                                    .child(Icon::new(IconName::Bot).large().text_color(primary))
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(foreground)
                                            .child("Welcome to AgentX"),
                                    )
                                    .child(
                                        div()
                                            .text_lg()
                                            .text_color(muted)
                                            .text_center()
                                            .child(workspace_label),
                                    ),
                            )
                            .child(composer)
                            .when_some(status, |this, status| {
                                this.child(
                                    h_flex()
                                        .gap_1p5()
                                        .items_center()
                                        .px_6()
                                        .child(
                                            Icon::new(IconName::CircleX)
                                                .xsmall()
                                                .text_color(danger),
                                        )
                                        .child(div().text_sm().text_color(danger).child(status)),
                                )
                            }),
                    ),
            )
            .children(Root::render_notification_layer(window, cx))
    }
}

impl EventEmitter<PanelEvent> for WelcomePanel {}

impl Focusable for WelcomePanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for WelcomePanel {
    fn panel_name(&self) -> &'static str {
        "WelcomePanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Welcome"
    }

    fn inner_padding(&self, _cx: &App) -> bool {
        false
    }
}
