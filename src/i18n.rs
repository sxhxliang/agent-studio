use gpui::App;
pub fn init(_cx: &mut App) {
    rust_i18n::i18n!("../locales");
    rust_i18n::set_locale("en");
}

pub fn change_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}
