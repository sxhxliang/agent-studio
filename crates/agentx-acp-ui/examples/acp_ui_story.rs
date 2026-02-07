#[path = "../stories/acp_ui_story.rs"]
mod acp_ui_story;

use acp_ui_story::AcpUiStory;
use gpui::{AppContext, Application, WindowOptions};
use gpui_component::{Root, TitleBar};

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            let window_options = WindowOptions {
                titlebar: Some(TitleBar::title_bar_options()),
                ..Default::default()
            };

            cx.open_window(window_options, |window, cx| {
                let view = AcpUiStory::view(window, cx);
                cx.new(|cx| Root::new(view, window, cx))
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
