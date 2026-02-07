use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, Size as UiSize, StyledExt as _, ThemeMode,
    ThemeRegistry,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    input::Input,
    scroll::ScrollableElement as _,
    stepper::{Stepper, StepperItem},
    switch::Switch,
    v_flex,
};
use rust_i18n::t;

use crate::{
    AppSettings, AppState,
    app::actions::{SelectLocale, SwitchTheme, SwitchThemeMode},
    assets::get_agent_icon,
    core::nodejs::NodeJsDetectionMode,
};

use super::state::NodeJsStatus;
use crate::workspace::DockWorkspace;

impl DockWorkspace {
    pub(in crate::workspace) fn render_startup(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let intro_icon = if self.startup_state.intro_completed {
            IconName::CircleCheck
        } else {
            IconName::Settings
        };

        let node_icon = match self.startup_state.nodejs_status {
            NodeJsStatus::Available { .. } => IconName::CircleCheck,
            NodeJsStatus::Unavailable { .. } => IconName::TriangleAlert,
            NodeJsStatus::Checking => IconName::LoaderCircle,
            NodeJsStatus::Idle => IconName::SquareTerminal,
        };

        let agent_icon = if self.startup_state.agents_ready() {
            IconName::CircleCheck
        } else if self.startup_state.agent_apply_error.is_some() {
            IconName::TriangleAlert
        } else {
            IconName::Bot
        };

        let workspace_icon = if self.startup_state.workspace_ready() {
            IconName::CircleCheck
        } else {
            IconName::Folder
        };

        let proxy_icon = if self.startup_state.proxy_ready() {
            IconName::CircleCheck
        } else if self.startup_state.proxy_apply_error.is_some() {
            IconName::TriangleAlert
        } else {
            IconName::Globe
        };

        let stepper = Stepper::new("startup-stepper")
            .w_full()
            .bg(cx.theme().background)
            .with_size(UiSize::Large)
            .selected_index(self.startup_state.step)
            .text_center(true)
            .items([
                StepperItem::new().icon(intro_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.preferences.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.preferences.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(node_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.nodejs.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.nodejs.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(agent_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.agents.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.agents.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(proxy_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.proxy.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.proxy.subtitle").to_string()),
                        ),
                ),
                StepperItem::new().icon(workspace_icon).child(
                    v_flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t!("startup.step.workspace.title").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(t!("startup.step.workspace.subtitle").to_string()),
                        ),
                ),
            ])
            .on_click(cx.listener(|this, step, _, cx| {
                if !this.startup_state.intro_completed && *step > 0 {
                    return;
                }
                this.startup_state.step = *step;
                cx.notify();
            }));

        let content = match self.startup_state.step {
            0 => self.render_preferences_step(cx),
            1 => self.render_nodejs_step(cx),
            2 => self.render_agents_step(cx),
            3 => self.render_proxy_step(cx),
            _ => self.render_workspace_step(cx),
        };

        let theme = cx.theme();
        let bg_color = theme.background;

        div()
            .flex_1()
            .size_full()
            .bg(theme.background)
            .flex()
            .items_center()
            .justify_center()
            .p_8()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(960.))
                    .gap_8()
                    .child(
                        div()
                            .text_size(px(36.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .text_center()
                            .child(t!("startup.title").to_string()),
                    )
                    .child(stepper)
                    .child(
                        div()
                            .w_full()
                            .min_h(px(400.))
                            .rounded(px(16.))
                            .bg(bg_color)
                            .shadow_lg()
                            .border_1()
                            .border_color(theme.border)
                            .p_8()
                            .child(content),
                    ),
            )
            .into_any_element()
    }

    fn render_preferences_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let current_locale = AppSettings::global(cx).locale.clone();
        let current_theme = cx.theme().theme_name().clone();
        let is_dark = cx.theme().mode.is_dark();
        let themes = ThemeRegistry::global(cx).sorted_themes();

        let locale_buttons = h_flex()
            .gap_2()
            .child(
                Button::new("startup-locale-en")
                    .label(t!("startup.preferences.locale.en").to_string())
                    .when(current_locale.as_ref() == "en", |btn| btn.primary())
                    .when(current_locale.as_ref() != "en", |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SelectLocale("en".into())), cx);
                    })),
            )
            .child(
                Button::new("startup-locale-zh")
                    .label(t!("startup.preferences.locale.zh_cn").to_string())
                    .when(current_locale.as_ref() == "zh-CN", |btn| btn.primary())
                    .when(current_locale.as_ref() != "zh-CN", |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SelectLocale("zh-CN".into())), cx);
                    })),
            );

        let theme_mode_buttons = h_flex()
            .gap_2()
            .child(
                Button::new("startup-theme-light")
                    .label(t!("startup.preferences.mode.light").to_string())
                    .when(!is_dark, |btn| btn.primary())
                    .when(is_dark, |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SwitchThemeMode(ThemeMode::Light)), cx);
                    })),
            )
            .child(
                Button::new("startup-theme-dark")
                    .label(t!("startup.preferences.mode.dark").to_string())
                    .when(is_dark, |btn| btn.primary())
                    .when(!is_dark, |btn| btn.outline())
                    .on_click(cx.listener(|_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SwitchThemeMode(ThemeMode::Dark)), cx);
                    })),
            );

        let mut theme_buttons = h_flex().w_full().gap_2().flex_wrap();
        for (idx, theme_config) in themes.iter().enumerate() {
            let name = theme_config.name.clone();
            let is_active = name == current_theme;
            theme_buttons = theme_buttons.child(
                Button::new(("startup-theme-btn", idx))
                    .label(name.clone())
                    .when(is_active, |btn| btn.icon(IconName::Check))
                    .when(!is_active, |btn| btn.outline())
                    .small()
                    .on_click(cx.listener(move |_, _ev, window, cx| {
                        window.dispatch_action(Box::new(SwitchTheme(name.clone())), cx);
                    })),
            );
        }

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.preferences.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.preferences.description").to_string()),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.preferences.language_label").to_string()),
                    )
                    .child(locale_buttons),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.preferences.theme_mode_label").to_string()),
                    )
                    .child(theme_mode_buttons),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.preferences.theme_label").to_string()),
                    )
                    .child(theme_buttons),
            );

        let actions = h_flex().gap_3().mt_6().justify_end().child(
            Button::new("startup-preferences-next")
                .label(t!("startup.preferences.continue").to_string())
                .primary()
                .on_click(cx.listener(|this, _ev, _window, cx| {
                    this.startup_state.intro_completed = true;
                    if this.startup_state.step == 0 {
                        this.startup_state.step = 1;
                    }
                    this.startup_state.advance_step_if_needed();
                    cx.notify();
                })),
        );

        content = content.child(actions);
        content.into_any_element()
    }

    fn render_nodejs_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.nodejs.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.nodejs.description").to_string()),
            );

        match &self.startup_state.nodejs_status {
            NodeJsStatus::Idle => {
                content = content.child(
                    div()
                        .mt_4()
                        .p_4()
                        .rounded(theme.radius)
                        .bg(theme.muted)
                        .text_color(theme.muted_foreground)
                        .child(t!("startup.nodejs.status.idle").to_string()),
                );
            }
            NodeJsStatus::Checking => {
                content = content.child(
                    h_flex()
                        .mt_4()
                        .p_4()
                        .gap_3()
                        .items_center()
                        .rounded(theme.radius)
                        .bg(theme.muted)
                        .child(
                            div()
                                .text_color(theme.accent_foreground)
                                .child(t!("startup.nodejs.status.checking").to_string()),
                        ),
                );
            }
            NodeJsStatus::Available { version, path } => {
                let detail = match (version, path) {
                    (Some(version), Some(path)) => t!(
                        "startup.nodejs.detail.version_path",
                        version = version,
                        path = path.display().to_string()
                    )
                    .to_string(),
                    (Some(version), None) => {
                        t!("startup.nodejs.detail.version", version = version).to_string()
                    }
                    (None, Some(path)) => t!(
                        "startup.nodejs.detail.path",
                        path = path.display().to_string()
                    )
                    .to_string(),
                    (None, None) => t!("startup.nodejs.detail.available").to_string(),
                };

                content = content.child(
                    v_flex()
                        .mt_4()
                        .p_4()
                        .gap_2()
                        .rounded(theme.radius)
                        .bg(theme.background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_color(theme.success_active)
                                .font_weight(FontWeight::MEDIUM)
                                .child(t!("startup.nodejs.success").to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme.muted_foreground)
                                .child(detail),
                        ),
                );
            }
            NodeJsStatus::Unavailable { message, hint } => {
                self.startup_state.nodejs_show_custom_input = true;

                content = content.child(
                    v_flex()
                        .mt_4()
                        .p_4()
                        .gap_2()
                        .rounded(theme.radius)
                        .bg(theme.background)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_color(theme.colors.danger_active)
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("⚠ {}", message)),
                        )
                        .when_some(hint.as_ref(), |this, hint| {
                            this.child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme.muted_foreground)
                                    .child(hint.clone()),
                            )
                        }),
                );
            }
        }

        if self.startup_state.nodejs_show_custom_input {
            let custom_path_input = self.startup_state.nodejs_custom_path_input.clone();
            let is_validating = self.startup_state.nodejs_custom_path_validating;

            content = content.child(
                v_flex()
                    .mt_4()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(t!("startup.nodejs.custom.title").to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(t!("startup.nodejs.custom.hint").to_string()),
                    )
                    .when_some(custom_path_input, |this, input| {
                        this.child(
                            h_flex().gap_2().child(Input::new(&input).w_full()).child(
                                Button::new("startup-nodejs-validate")
                                    .label(if is_validating {
                                        t!("startup.nodejs.custom.validating").to_string()
                                    } else {
                                        t!("startup.nodejs.custom.validate").to_string()
                                    })
                                    .outline()
                                    .disabled(is_validating)
                                    .on_click(cx.listener(|this, _ev, window, cx| {
                                        this.validate_custom_nodejs_path(window, cx);
                                    })),
                            ),
                        )
                    })
                    .when_some(
                        self.startup_state.nodejs_custom_path_error.clone(),
                        |this, error| {
                            this.child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(theme.colors.danger_active)
                                    .child(error),
                            )
                        },
                    ),
            );
        }

        let mut actions = h_flex().gap_3().mt_6().justify_between();

        let show_custom = self.startup_state.nodejs_show_custom_input;
        let left_actions = h_flex()
            .gap_2()
            .child(
                Button::new("startup-nodejs-recheck")
                    .label(t!("startup.nodejs.action.recheck").to_string())
                    .outline()
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.start_nodejs_check(window, cx, NodeJsDetectionMode::Full);
                    })),
            )
            .child(
                Button::new("startup-nodejs-manual")
                    .label(if show_custom {
                        t!("startup.nodejs.action.collapse").to_string()
                    } else {
                        t!("startup.nodejs.action.manual").to_string()
                    })
                    .ghost()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.nodejs_show_custom_input =
                            !this.startup_state.nodejs_show_custom_input;
                        cx.notify();
                    })),
            );

        let right_actions = if self.startup_state.nodejs_ready() {
            h_flex().child(
                Button::new("startup-nodejs-next")
                    .label(t!("startup.nodejs.action.next").to_string())
                    .primary()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.step = 2;
                        cx.notify();
                    })),
            )
        } else {
            h_flex().child(
                Button::new("startup-nodejs-skip")
                    .label(t!("startup.nodejs.action.skip").to_string())
                    .ghost()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.nodejs_skipped = true;
                        this.startup_state.advance_step_if_needed();
                        cx.notify();
                    })),
            )
        };

        actions = actions.child(left_actions).child(right_actions);

        content.child(actions).into_any_element()
    }

    fn render_agents_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_6()
            .child(
                div().absolute().top_0().right_0().child(
                    Button::new("startup-close")
                        .ghost()
                        .icon(IconName::Close)
                        .on_click(cx.listener(|this, _ev, _, cx| {
                            this.startup_state.agent_applied = true;
                            this.startup_state.advance_step_if_needed();
                            cx.notify();
                        })),
                ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(t!("startup.agents.title").to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(theme.muted_foreground)
                            .line_height(rems(1.5))
                            .child(t!("startup.agents.description").to_string()),
                    ),
            );

        if let Some(error) = &self.startup_state.agent_load_error {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_color(theme.colors.danger_foreground)
                    .child(format!("⚠ {}", error)),
            );
        }

        if self.startup_state.agent_choices.is_empty() {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(t!("startup.agents.empty").to_string()),
            );
        } else {
            let disabled = self.startup_state.agent_apply_in_progress;

            let mut list = v_flex().w_full().gap_0();

            for (idx, choice) in self.startup_state.agent_choices.iter().enumerate() {
                let name = choice.name.clone();
                let checked = choice.enabled;

                let icon = get_agent_icon(&name);

                list = list.child(
                    h_flex()
                        .w_full()
                        .py_2()
                        .px_3()
                        .gap_2()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(theme.border)
                        .when(idx == 0, |this| this.border_t_1())
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    Checkbox::new(("startup-agent-check", idx))
                                        .checked(checked)
                                        .disabled(disabled)
                                        .with_size(UiSize::Small)
                                        .on_click(cx.listener(move |this, checked, _, cx| {
                                            if let Some(choice) =
                                                this.startup_state.agent_choices.get_mut(idx)
                                            {
                                                choice.enabled = *checked;
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    div()
                                        .w(px(24.))
                                        .h(px(24.))
                                        .rounded(px(6.))
                                        .bg(theme.background)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Icon::new(icon)
                                                .size(px(14.))
                                                .text_color(theme.foreground),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .child(name.clone()),
                                ),
                        )
                        .child(
                            Switch::new(("startup-agent-switch", idx))
                                .checked(checked)
                                .disabled(disabled)
                                .with_size(UiSize::Small)
                                .on_click(cx.listener(move |this, checked, _, cx| {
                                    if let Some(choice) =
                                        this.startup_state.agent_choices.get_mut(idx)
                                    {
                                        choice.enabled = *checked;
                                        cx.notify();
                                    }
                                })),
                        ),
                );
            }

            let scroll_handle = &self.startup_state.agent_list_scroll_handle;
            let scrollable_list = div()
                .id("agent-list-scroll-container")
                .w_full()
                .max_h(px(280.))
                .min_h_0()
                .rounded(px(8.))
                .border_1()
                .border_color(theme.border)
                .overflow_y_scroll()
                .track_scroll(scroll_handle)
                .child(list);

            content = content.child(scrollable_list);
        }

        if AppState::global(cx).agent_config_service().is_none() {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(t!("startup.agents.service_loading").to_string()),
            );
        }

        if let Some(error) = &self.startup_state.agent_apply_error {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.colors.danger_foreground)
                    .child(format!("⚠ {}", error)),
            );
        }

        let service_ready = AppState::global(cx).agent_config_service().is_some();
        let apply_label = if self.startup_state.agent_apply_in_progress {
            t!("startup.agents.apply.in_progress")
        } else {
            t!("startup.agents.apply.ready")
        };

        let enabled_count = self
            .startup_state
            .agent_choices
            .iter()
            .filter(|c| c.enabled)
            .count();

        let actions = h_flex()
            .mt_6()
            .pt_6()
            .border_t_1()
            .border_color(theme.border)
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_size(px(14.))
                    .text_color(theme.colors.muted_foreground)
                    .child(
                        t!(
                            "startup.agents.footer.selected",
                            selected = enabled_count,
                            total = self.startup_state.agent_choices.len()
                        )
                        .to_string(),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("startup-agent-skip")
                            .label(t!("startup.agents.action.skip").to_string())
                            .outline()
                            .on_click(cx.listener(|this, _ev, _, cx| {
                                this.startup_state.agent_applied = true;
                                this.startup_state.advance_step_if_needed();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("startup-agent-apply")
                            .label(apply_label.to_string())
                            .primary()
                            .disabled(!service_ready || self.startup_state.agent_apply_in_progress)
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.apply_agent_selection(window, cx);
                            })),
                    ),
            );

        content.child(actions).into_any_element()
    }

    fn render_proxy_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let http_input = self.startup_state.proxy_http_input.clone();
        let https_input = self.startup_state.proxy_https_input.clone();
        let all_input = self.startup_state.proxy_all_input.clone();

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.proxy.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.proxy.description").to_string()),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        Switch::new("startup-proxy-enabled")
                            .checked(self.startup_state.proxy_enabled)
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.startup_state.proxy_enabled = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .text_color(theme.foreground)
                            .child(t!("startup.proxy.enable").to_string()),
                    ),
            );

        if let Some(http_input) = http_input {
            content = content.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child("HTTP_PROXY"),
                    )
                    .child(
                        Input::new(&http_input)
                            .disabled(!self.startup_state.proxy_enabled)
                            .w_full(),
                    ),
            );
        }

        if let Some(https_input) = https_input {
            content = content.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child("HTTPS_PROXY"),
                    )
                    .child(
                        Input::new(&https_input)
                            .disabled(!self.startup_state.proxy_enabled)
                            .w_full(),
                    ),
            );
        }

        if let Some(all_input) = all_input {
            content = content.child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child("ALL_PROXY"),
                    )
                    .child(
                        Input::new(&all_input)
                            .disabled(!self.startup_state.proxy_enabled)
                            .w_full(),
                    ),
            );
        }

        if let Some(error) = &self.startup_state.proxy_apply_error {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(8.))
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.colors.danger_foreground)
                    .child(format!("⚠ {}", error)),
            );
        }

        let apply_label = if self.startup_state.proxy_apply_in_progress {
            t!("startup.proxy.apply.in_progress")
        } else {
            t!("startup.proxy.apply.ready")
        };

        let actions = h_flex()
            .mt_6()
            .pt_6()
            .border_t_1()
            .border_color(theme.border)
            .justify_between()
            .items_center()
            .child(
                Button::new("startup-proxy-skip")
                    .label(t!("startup.proxy.action.skip").to_string())
                    .outline()
                    .on_click(cx.listener(|this, _ev, _, cx| {
                        this.startup_state.proxy_applied = true;
                        this.startup_state.advance_step_if_needed();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("startup-proxy-apply")
                    .label(apply_label.to_string())
                    .primary()
                    .disabled(self.startup_state.proxy_apply_in_progress)
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.apply_proxy_config(window, cx);
                    })),
            );

        content.child(actions).into_any_element()
    }

    fn render_workspace_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let mut content = v_flex()
            .gap_4()
            .child(
                div()
                    .text_size(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(t!("startup.workspace.title").to_string()),
            )
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .line_height(rems(1.5))
                    .child(t!("startup.workspace.description").to_string()),
            );

        if let Some(path) = &self.startup_state.workspace_path {
            content = content.child(
                v_flex()
                    .mt_4()
                    .p_4()
                    .gap_2()
                    .rounded(theme.radius)
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_color(theme.success_active)
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("startup.workspace.status.selected").to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.muted_foreground)
                            .child(format!("{}", path.display())),
                    ),
            );
        } else {
            content = content.child(
                div()
                    .mt_4()
                    .p_4()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(t!("startup.workspace.status.not_selected").to_string()),
            );
        }

        if let Some(error) = &self.startup_state.workspace_error {
            content = content.child(
                div()
                    .mt_4()
                    .p_4()
                    .rounded(theme.radius)
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .text_color(theme.colors.danger_active)
                    .child(format!("⚠ {}", error)),
            );
        }

        if self.startup_state.workspace_loading {
            content = content.child(
                div()
                    .mt_4()
                    .p_4()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .text_color(theme.accent_foreground)
                    .child(t!("startup.workspace.status.loading").to_string()),
            );
        }

        let pick_label = if self.startup_state.workspace_selected {
            t!("startup.workspace.action.repick").to_string()
        } else {
            t!("startup.workspace.action.pick").to_string()
        };

        let actions = h_flex()
            .mt_6()
            .pt_6()
            .border_t_1()
            .border_color(theme.border)
            .justify_between()
            .items_center()
            .child(
                Button::new("startup-workspace-pick")
                    .label(pick_label)
                    .outline()
                    .disabled(self.startup_state.workspace_loading)
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.open_workspace_folder(window, cx);
                    })),
            )
            .child(
                h_flex()
                    .gap_3()
                    .when(self.startup_state.workspace_selected, |this| {
                        this.child(
                            Button::new("startup-workspace-finish")
                                .label(t!("startup.workspace.action.finish").to_string())
                                .primary()
                                .disabled(self.startup_state.workspace_loading)
                                .on_click(cx.listener(|this, _ev, window, cx| {
                                    this.startup_state.workspace_selected = true;
                                    this.startup_state.workspace_checked = true;

                                    window.refresh();
                                    cx.notify();
                                })),
                        )
                    }),
            );

        content.child(actions).into_any_element()
    }
}
