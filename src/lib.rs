mod app;
mod clock;
mod config;
mod drives;
mod format;
mod icons;
mod keybinds;
mod notice;
mod places;
mod rules;
mod session;
mod settings;
mod system_icons;
mod ui;
mod vfs;
mod views;

#[cfg(test)]
mod tests;

use rune::AppSpec;

// The faces come from the terminal crate's own list, which owns why the theme's third face is
// left out: see `rune_tty::FONTS`.
/// Opens Sheath's window. Shared by the desktop binary and the browser entry point, so both
/// builds start the same app from the same spec.
pub fn launch() {
    rune::set_app_id("dev.sheath.Sheath");
    let (fonts, families) = rune_tty::fonts::spec();
    rune::run(AppSpec {
        min_size: Some((1000.0, 600.0)),
        fonts,
        families,
        on_key: Some(app::on_key),
        poll: Some(app::poll),
        setup: Some(app::setup),
        scenes: &[
            ("settings", |s: &mut app::App| {
                s.open_tab(app::SettingsTab::General)
            }),
            ("settings-set", |s: &mut app::App| {
                s.toggle_hidden();
                s.open_tab(app::SettingsTab::General)
            }),
        ],
        ..AppSpec::new("Sheath", (1180.0, 760.0), app::App::new, ui::view)
    });
}

/// Loads the KDL files OPFS kept from the last visit, before the web build opens its window.
#[cfg(target_arch = "wasm32")]
pub async fn hydrate_web() {
    vfs::hydrate().await;
}
