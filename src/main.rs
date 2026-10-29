//! Sheath: a small file manager built entirely with Rune's view, layout, input, animation and
//! rendering layers.

mod app;
mod config;
mod format;
mod icons;
mod keybinds;
mod keycaps;
mod recorder;
mod settings;
mod ui;

#[cfg(test)]
mod tests;

use rune::AppSpec;

fn main() {
    rune::set_app_id("dev.sheath.Sheath");
    rune::run(AppSpec {
        min_size: Some((1000.0, 600.0)),
        fonts: rune_theme_opencode::FONTS,
        families: rune_theme_opencode::FAMILIES,
        on_key: Some(app::on_key),
        poll: Some(app::poll),
        ..AppSpec::new("Sheath", (1180.0, 760.0), app::App::new, ui::view)
    });
}
