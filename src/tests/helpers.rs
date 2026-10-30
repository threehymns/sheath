//! Shared fixtures and drivers: the folders the tests open, and the runtime helpers that
//! click, key and drag in them.

use std::fs;

use rune::text::TextSystem;
use rune::{Env, Key, KeyEvent, Mods, PhysicalInput, PhysicalKeyCode, Runtime, WheelEvent};
use tempfile::TempDir;

use crate::app::App;
use crate::keybinds::{Action, Keybinds};
use crate::settings::Settings;
use crate::ui;

pub(super) fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    populate(&dir);
    dir
}

/// A folder with `count` files, so its list is taller than the window and scrolls.
pub(super) fn long_folder(count: usize) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..count {
        fs::write(dir.path().join(format!("file{i:02}.txt")), "x").unwrap();
    }
    dir
}

pub(super) fn populate(dir: &TempDir) {
    fs::create_dir(dir.path().join("alpha")).unwrap();
    fs::create_dir(dir.path().join(".secret")).unwrap();
    fs::write(dir.path().join("file2.txt"), "two").unwrap();
    fs::write(dir.path().join("file10.txt"), "ten!").unwrap();
    fs::write(dir.path().join("notes.md"), "# Notes\n\nHello preview.\n").unwrap();
}

pub(super) fn app(dir: &TempDir) -> App {
    let mut app = App::open(dir.path().to_path_buf());
    // The defaults, not whatever the machine running the tests has saved.
    app.store = crate::config::Store::none();
    app.keybinds = Keybinds::defaults();
    app.recorders = Keybinds::recorders();
    app.config = Settings::defaults();
    app.views = crate::views::Views::default();
    app.rules = crate::rules::Rules::default();
    app.rule_error = None;
    // Opening applied whatever the machine saved, so the list view is put back by hand.
    app.view = crate::app::ViewMode::List;
    app
}

/// An app whose keybinds, settings, rules, and remembered state live in `dir`, so a test never
/// touches the real config folder.
pub(super) fn app_in_config_dir(dir: &TempDir) -> (App, std::path::PathBuf) {
    let path = dir.path().join("keybinds.kdl");
    let mut app = app(dir);
    app.store = crate::config::Store::at(dir.path().to_path_buf());
    app.keybinds = Keybinds::from_doc(app.store.doc(crate::config::KEYBINDS));
    app.recorders = Keybinds::recorders();
    app.config = Settings::from_doc(app.store.doc(crate::config::CONFIG));
    app.views = crate::views::Views::at(dir.path().join("views.kdl"));
    app.reload_rules();
    app.reload();
    (app, path)
}

/// The settings a fresh read of `dir` finds, as the next launch would.
pub(super) fn saved_settings(dir: &TempDir) -> Settings {
    let store = crate::config::Store::at(dir.path().to_path_buf());
    Settings::from_doc(store.doc(crate::config::CONFIG))
}

/// The keybinds a fresh read of `dir` finds, as the next launch would.
pub(super) fn saved_keybinds(dir: &TempDir) -> Keybinds {
    let store = crate::config::Store::at(dir.path().to_path_buf());
    Keybinds::from_doc(store.doc(crate::config::KEYBINDS))
}

pub(super) fn names(app: &App) -> Vec<&str> {
    app.visible
        .iter()
        .map(|&i| app.entries[i].name.as_str())
        .collect()
}

pub(super) fn runtime(app: App) -> Runtime<App> {
    let text = TextSystem::new(rune_theme_opencode::FONTS, rune_theme_opencode::FAMILIES);
    let mut rt = Runtime::with_text(app, ui::view, (1180.0, 760.0), Env::new(true, 2.0), text);
    rt.on_key = Some(crate::app::on_key);
    rt.poll = Some(crate::app::poll);
    rt
}

pub(super) fn right_click(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).expect("the row is on screen");
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(0.1);
}

pub(super) fn click(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).expect("the control is on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
}

/// Presses and releases `key`, the way a keyboard sends it.
pub(super) fn press(rt: &mut Runtime<App>, key: PhysicalKeyCode) {
    for pressed in [true, false] {
        rt.physical_input(PhysicalInput::Key {
            key,
            mods: Mods::default(),
            pressed,
            repeat: false,
        });
        rt.frame(0.1);
    }
}

/// The event a binding's chord arrives as, with the key the shortcut names.
pub(super) fn event_for(app: &App, action: Action, key: Key) -> KeyEvent {
    let chord = app.keybinds.chord(action).expect("the action is bound");
    KeyEvent {
        key,
        mods: chord.modifiers(),
    }
}

/// Where the list is scrolled to, as the viewport last said.
pub(super) fn scroll_offset(rt: &Runtime<App>) -> f32 {
    rt.state.scroll_told.get().offset[1]
}

/// A fast trackpad: four precise deltas of 9 points between frames, partway down the list.
pub(super) fn trackpad_burst(rt: &mut Runtime<App>, frames: usize) -> Vec<f32> {
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    let mut at = vec![scroll_offset(rt)];
    for _ in 0..frames {
        for _ in 0..4 {
            rt.wheel_event(WheelEvent::new(0.0, -9.0).precise(true).at(x, y));
        }
        rt.frame(1.0 / 60.0);
        at.push(scroll_offset(rt));
    }
    at
}

pub(super) fn command_bar_event(app: &App, shifted: bool) -> KeyEvent {
    let mut mods = app
        .keybinds
        .chord(Action::CommandBar)
        .expect("the command bar is bound")
        .modifiers();
    mods.shift |= shifted;
    KeyEvent {
        key: Key::Command('p'),
        mods,
    }
}

/// Clicks `key` while `mods` are held, as a Shift- or Cmd-click.
pub(super) fn click_with(rt: &mut Runtime<App>, key: &str, mods: Mods) {
    rt.physical_input(PhysicalInput::Modifiers(mods));
    click(rt, key);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(0.1);
}

pub(super) fn shift() -> Mods {
    Mods {
        shift: true,
        ..Mods::default()
    }
}

pub(super) fn command() -> Mods {
    Mods {
        cmd: true,
        ..Mods::default()
    }
}

pub(super) fn control_shift() -> Mods {
    Mods {
        ctrl: true,
        shift: true,
        ..Mods::default()
    }
}

/// Drags a box from the list's empty space up to a row, holding `mods`.
pub(super) fn drag_box(rt: &mut Runtime<App>, to: &str, mods: Mods) {
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    let (_, y1) = rt.find(to).expect("the row is on screen");
    assert!(y1 < y, "the box starts below the rows");
    rt.physical_input(PhysicalInput::Modifiers(mods));
    rt.pointer_down(x, y);
    rt.frame(0.05);
    rt.pointer_move(x, y1);
    rt.frame(0.05);
    rt.pointer_up(x, y1);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(0.1);
}

/// Drags one row's press onto another row with the pointer, as a file drag does.
pub(super) fn drag_row(rt: &mut Runtime<App>, from: &str, to: &str) {
    let (x0, y0) = rt.find(from).expect("the first row is on screen");
    let (x1, y1) = rt.find(to).expect("the second row is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x1, y1);
    rt.frame(0.05);
    rt.pointer_up(x1, y1);
    rt.frame(0.1);
}
