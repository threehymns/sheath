mod app;
mod clock;
mod config;
mod corner;
mod format;
mod icons;
mod keybinds;
mod keycaps;
mod notice;
mod places;
mod recorder;
mod rules;
mod session;
mod settings;
mod term;
mod ui;
mod vfs;
mod views;

#[cfg(test)]
mod tests;

use rune::AppSpec;

/// The faces the app draws with: Inter (variable, so every weight) and IBM Plex Mono Regular
/// for sizes and the inspector. The theme also ships Plex Mono Medium, which nothing asks for —
/// its `mono` style is weight 400 — and carrying it costs the wasm build about 157 KB, 55 KB
/// compressed. The order is `rune_theme_opencode::FONTS`: Inter, Plex Mono Regular, Plex Mono
/// Medium, pinned by the Rune revision in Cargo.toml.
const FONTS: &[&[u8]] = &[rune_theme_opencode::FONTS[0], rune_theme_opencode::FONTS[2]];

/// Opens Sheath's window. Shared by the desktop binary and the browser entry point, so both
/// builds start the same app from the same spec.
pub fn launch() {
    rune::set_app_id("dev.sheath.Sheath");
    rune::run(AppSpec {
        min_size: Some((1000.0, 600.0)),
        fonts: FONTS,
        families: rune_theme_opencode::FAMILIES,
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
