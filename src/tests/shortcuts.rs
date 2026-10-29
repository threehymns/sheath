//! The keybind table: its defaults, its file, and recording a chord.

use super::helpers::*;
use crate::app::{SettingsTab, SheetKind};
use crate::keybinds::{Action, Keybinds};
use rune::{Key, KeyEvent, Mods, ShortcutError, Tx};
use std::fs;

#[test]
fn the_default_shortcuts_run_their_commands() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:notes.md");

    let rename = event_for(&app, Action::Rename, Key::Function(2));
    assert!(crate::app::on_key(&mut app, &rename, &mut tx));
    assert_eq!(app.sheet.as_ref().map(|s| s.kind), Some(SheetKind::Rename));
    app.close_sheet();

    fs::write(dir.path().join("late.txt"), "late").unwrap();
    let refresh = event_for(&app, Action::Refresh, Key::Command('r'));
    assert!(crate::app::on_key(&mut app, &refresh, &mut tx));
    assert!(names(&app).contains(&"late.txt"));

    let settings = event_for(&app, Action::OpenSettings, Key::Command(','));
    assert!(crate::app::on_key(&mut app, &settings, &mut tx));
    assert_eq!(app.settings, Some(SettingsTab::General));
}

#[test]
fn a_recorded_shortcut_replaces_the_default() {
    let dir = fixture();
    let (mut app, path) = app_in_config_dir(&dir);
    let mut tx = Tx::default();
    app.select_id("f:notes.md");

    let f5: rune::Shortcut = "f5".parse().unwrap();
    app.set_binding(Action::Rename, Some(&f5)).unwrap();
    assert!(path.exists(), "the change is saved as it's made");

    let old = KeyEvent {
        key: Key::Function(2),
        mods: Mods::default(),
    };
    assert!(!crate::app::on_key(&mut app, &old, &mut tx));
    assert!(app.sheet.is_none());

    let new = KeyEvent {
        key: Key::Function(5),
        mods: Mods::default(),
    };
    assert!(crate::app::on_key(&mut app, &new, &mut tx));
    assert_eq!(app.sheet.as_ref().map(|s| s.kind), Some(SheetKind::Rename));
}

#[test]
fn a_chord_another_command_uses_is_refused() {
    let dir = fixture();
    let (mut app, _path) = app_in_config_dir(&dir);
    let refresh = app.keybinds.chord(Action::Refresh).cloned().unwrap();
    let back = app.keybinds.chord(Action::Back).cloned().unwrap();

    assert_eq!(
        app.set_binding(Action::Back, Some(&refresh)),
        Err(ShortcutError::Conflict)
    );
    assert_eq!(app.keybinds.chord(Action::Back), Some(&back));
}

#[test]
fn keybinds_round_trip_through_their_own_kdl_file() {
    let dir = fixture();
    let (mut app, path) = app_in_config_dir(&dir);
    let f5: rune::Shortcut = "f5".parse().unwrap();
    app.set_binding(Action::Rename, Some(&f5)).unwrap();
    app.set_binding(Action::Move, None).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("keybinds {"), "the file is KDL: {text}");
    assert!(text.contains("rename f5"), "the chord is written: {text}");
    assert!(
        text.contains("move \"\""),
        "a cleared shortcut stays cleared: {text}"
    );

    let saved = Keybinds::at(path.clone());
    assert_eq!(saved.error, None);
    assert_eq!(saved.chord(Action::Rename), Some(&f5));
    assert_eq!(saved.chord(Action::Move), None);
    assert_eq!(
        saved.chord(Action::Refresh),
        Keybinds::defaults().chord(Action::Refresh)
    );
}

#[test]
fn a_broken_keybinds_file_keeps_the_defaults_and_says_so() {
    let dir = fixture();
    let path = dir.path().join("keybinds.kdl");
    fs::write(&path, "keybinds { rename \"f5\"").unwrap();

    let binds = Keybinds::at(path);
    assert!(
        binds
            .error
            .as_deref()
            .is_some_and(|e| e.contains("Couldn’t read"))
    );
    assert_eq!(
        binds.chord(Action::Rename),
        Keybinds::defaults().chord(Action::Rename)
    );
}
