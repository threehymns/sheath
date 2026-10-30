//! The settings panel, its switches, and the files they save to.

use super::helpers::*;
use crate::app::{App, SettingsTab};
use crate::keybinds::{Action, Keybinds};
use crate::settings::Settings;
use rune::{Key, KeyEvent, Mods, PhysicalInput, PhysicalKeyCode};
use std::fs;

#[test]
fn done_sits_in_the_dialogs_corner_outside_the_panel() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");

    let (dialog, _) = rt.presented("settings-dialog").expect("the dialog is up");
    let (panel, _) = rt.presented("settings-panel").expect("the panel is up");
    let (done, _) = rt.presented("close-settings").expect("Done is up");
    let (cut, _) = rt
        .presented("corner-cut")
        .expect("the panel's corner is cut");
    // The inset the panel keeps from the dialog's edge, so Done shares the dialog's corner.
    let inset = dialog[0] + dialog[2] - (panel[0] + panel[2]);
    assert!(
        (done[0] + done[2] - (dialog[0] + dialog[2] - inset)).abs() < 0.6
            && (done[1] - (dialog[1] + inset)).abs() < 0.6,
        "Done sits in the dialog's top-right corner: {done:?} against {dialog:?}"
    );
    assert!(
        cut[0] < done[0] && cut[1] + cut[3] > done[1] + done[3],
        "the cut reaches around Done's left and bottom edges: {cut:?} against {done:?}"
    );
    assert!(
        (cut[0] + cut[2] - (panel[0] + panel[2])).abs() < 0.6,
        "the cut ends on the panel's right edge: {cut:?} against {panel:?}"
    );
    // The bite, its gap around Done, and room for the panel's own 8 pt corner radius where it
    // rounds down onto the panel's right edge.
    assert!(
        (cut[2] - (done[2] + 8.0 + 8.0)).abs() < 0.6,
        "the cut is Done plus its gap and the panel corner's radius: {cut:?} against {done:?}"
    );
    assert!(
        rt.find("settings-footer").is_none(),
        "the page has no footer, since Done left it"
    );
    assert!(
        rt.find("settings-version").is_some(),
        "the version sits at the rail's foot"
    );
}

#[test]
fn the_settings_panel_has_a_sidebar_of_categories() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    assert_eq!(rt.state.settings, Some(SettingsTab::General));
    assert!(rt.find("settings").is_some());
    assert!(
        rt.find("copy-keybinds-path").is_some(),
        "General lists where the keybinds are saved"
    );
    for tab in SettingsTab::ALL {
        assert!(
            rt.find(&format!("settings-tab-{}", tab.id())).is_some(),
            "{} is in the sidebar",
            tab.label()
        );
    }

    click(&mut rt, "settings-tab-keybinds");
    assert_eq!(rt.state.settings, Some(SettingsTab::Keybinds));
    for action in Action::ALL {
        assert!(
            rt.find(&format!("record-{}-field", action.id())).is_some(),
            "{} has a recorder",
            action.id()
        );
    }
    assert!(
        rt.find("settings-footer").is_none(),
        "Keybinds has no Restore Defaults footer"
    );

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.settings.is_none(), "Escape closes the panel");
    assert!(rt.find("settings").is_none());

    click(&mut rt, "open-settings");
    click(&mut rt, "close-settings");
    assert!(rt.state.settings.is_none());
}

#[test]
fn the_settings_modal_takes_presses_from_the_browser() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    // The row is still laid out under the dialog; a press there lands on the settings rather
    // than the browser.
    let (x, y) = rt.find("f:alpha").expect("the row is under the dialog");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert!(rt.state.selected.is_none(), "no row was clicked");

    // A press outside the dialog lands on the scrim, which dismisses the settings.
    rt.pointer_down(8.0, 8.0);
    rt.pointer_up(8.0, 8.0);
    rt.frame(0.1);
    assert!(rt.state.selected.is_none(), "no row was clicked");
    assert!(
        rt.state.settings.is_none(),
        "the scrim dismissed the settings"
    );
}

#[test]
fn the_general_category_toggles_what_the_browser_shows() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    assert!(!rt.state.show_hidden);
    click(&mut rt, "setting-hidden");
    assert!(rt.state.show_hidden, "the switch shows hidden files");
    click(&mut rt, "setting-inspector");
    assert!(
        !rt.state.inspector.is_open(),
        "the switch hides the inspector"
    );
    assert!(rt.state.config.scroll_inertia, "inertia starts on");
    click(&mut rt, "setting-inertia");
    assert!(
        !rt.state.config.scroll_inertia,
        "the switch turns inertia off"
    );
    assert!(rt.state.config.overscroll, "overscroll starts on");
    click(&mut rt, "setting-overscroll");
    assert!(
        !rt.state.config.overscroll,
        "the switch turns overscroll off"
    );

    let config = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(config.contains("show-hidden #true"), "{config}");
    assert!(config.contains("inspector #false"), "{config}");
    assert!(config.contains("scroll-inertia #false"), "{config}");
    assert!(config.contains("overscroll #false"), "{config}");
}

#[test]
fn saved_settings_load_at_launch() {
    let dir = fixture();
    let path = dir.path().join("config.kdl");
    fs::write(
        &path,
        "settings {\n    show-hidden #true\n    inspector #false\n}\n",
    )
    .unwrap();

    let saved = saved_settings(&dir);
    assert!(saved.show_hidden);
    assert!(!saved.inspector);

    let app = App::open_with(dir.path().to_path_buf(), saved);
    assert!(app.show_hidden);
    assert!(!app.inspector.is_open());
    assert!(
        names(&app).contains(&".secret"),
        "the saved choice shows hidden files"
    );
}

#[test]
fn saved_scroll_settings_load_at_launch() {
    let dir = fixture();
    let path = dir.path().join("config.kdl");
    fs::write(
        &path,
        "settings {\n    scroll-inertia #true\n    overscroll #true\n}\n",
    )
    .unwrap();

    let saved = saved_settings(&dir);
    assert!(saved.scroll_inertia);
    assert!(saved.overscroll);
    assert!(!saved.show_hidden, "the rest keep their defaults");
    assert!(saved.inspector);

    let app = App::open_with(dir.path().to_path_buf(), saved);
    assert!(app.config.scroll_inertia);
    assert!(app.config.overscroll);
}

#[test]
fn settings_and_keybinds_save_to_separate_files() {
    let dir = fixture();
    let (mut app, keybinds_path) = app_in_config_dir(&dir);
    app.toggle_hidden();
    app.toggle_inspector();

    let config = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(config.contains("show-hidden #true"), "{config}");
    assert!(
        !config.contains("rename") && !config.contains("open-settings"),
        "keybinds stay out of the settings file: {config}"
    );
    assert!(
        !keybinds_path.exists(),
        "changing a setting doesn't write the keybinds file"
    );

    app.set_binding(Action::Rename, Some(&"f5".parse().unwrap()))
        .unwrap();
    let keybinds = fs::read_to_string(&keybinds_path).unwrap();
    assert!(
        !keybinds.contains("show-hidden") && !keybinds.contains("inspector"),
        "settings stay out of the keybinds file: {keybinds}"
    );
}

#[test]
fn a_broken_config_file_keeps_the_defaults_and_says_so() {
    let dir = fixture();
    let path = dir.path().join("config.kdl");
    fs::write(&path, "settings { show-hidden #true").unwrap();

    let store = crate::config::Store::at(dir.path().to_path_buf());
    assert!(
        store
            .error(crate::config::CONFIG)
            .is_some_and(|e| e.contains("Couldn’t read")),
        "{:?}",
        store.error(crate::config::CONFIG)
    );
    let saved = Settings::from_doc(store.doc(crate::config::CONFIG));
    assert!(!saved.show_hidden);
    assert!(saved.inspector);
    assert!(saved.scroll_inertia);
    assert!(saved.overscroll);
}

#[test]
fn recording_a_chord_in_the_settings_binds_and_saves_it() {
    let dir = fixture();
    let (app, path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-keybinds");
    click(&mut rt, "record-rename-field");
    assert!(rt.find("record-rename-field").is_some());

    rt.physical_input(PhysicalInput::Key {
        key: PhysicalKeyCode::F5,
        mods: Mods::default(),
        pressed: true,
        repeat: false,
    });
    rt.frame(0.1);
    rt.physical_input(PhysicalInput::Key {
        key: PhysicalKeyCode::F5,
        mods: Mods::default(),
        pressed: false,
        repeat: false,
    });
    rt.frame(0.1);

    let f5: rune::Shortcut = "f5".parse().unwrap();
    assert_eq!(rt.state.keybinds.chord(Action::Rename), Some(&f5));
    let saved = saved_keybinds(&dir);
    assert_eq!(saved.chord(Action::Rename), Some(&f5), "the file has it");
}

#[test]
fn the_recorder_shows_the_bound_chord_as_a_keycap() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-keybinds");

    let first = rt.state.keybinds.keycaps(Action::Rename)[0].clone();
    let drawn = rt.elements().iter().any(|e| {
        e.path.contains("/record-rename-field/")
            && e.path.ends_with("/keycap0/cap/face/label")
            && e.text.as_deref() == Some(first.as_str())
    });
    assert!(
        drawn,
        "the rename recorder draws its chord {first:?} on a key in a tray"
    );
}

#[test]
fn the_recorder_cancels_with_escape_and_clears_with_backspace() {
    let dir = fixture();
    let (app, path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-keybinds");

    let f2: rune::Shortcut = "f2".parse().unwrap();
    click(&mut rt, "record-rename-field");
    press(&mut rt, PhysicalKeyCode::Escape);
    assert_eq!(rt.state.keybinds.chord(Action::Rename), Some(&f2));
    assert!(!rt.state.recorders[&Action::Rename].is_recording());

    click(&mut rt, "record-rename-field");
    press(&mut rt, PhysicalKeyCode::Backspace);
    assert_eq!(rt.state.keybinds.chord(Action::Rename), None);
    assert_eq!(
        saved_keybinds(&dir).chord(Action::Rename),
        None,
        "the file has the cleared chord"
    );
}

/// Hex's settings: the rail sits on the window's background, and each page's rows live in a
/// card beside it.
#[test]
fn the_settings_pages_group_their_rows_into_a_card_beside_the_rail() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");

    let elements = rt.elements();
    let find = |key: &str| {
        elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("{key} is drawn"))
    };
    let rail = find("settings-sidebar").rect;
    let panel = find("settings-panel").rect;
    assert_eq!(
        (panel[0] - rail[0], panel[1] - rail[1]),
        (rail[2], 8.0),
        "the panel starts where the rail ends, inset 8 pt from the top"
    );
    let card = find("settings-browser-card");
    let toggle = find("setting-hidden");
    assert!(
        toggle.path.starts_with(&card.path),
        "the hidden-files switch sits in the Browser card"
    );
}

/// A shortcut the file names can be reset like a setting: the undo takes its line out of
/// `keybinds.kdl`, and an action the file leaves alone has no undo at all.
#[test]
fn resetting_a_keybind_takes_its_line_out_of_the_file() {
    let dir = fixture();
    let (app, path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-keybinds");
    assert!(
        rt.find("reset-binding-rename").is_none(),
        "the built-in chord needs no reset"
    );

    click(&mut rt, "record-rename-field");
    press(&mut rt, PhysicalKeyCode::F5);
    assert!(rt.state.keybinds.is_custom(Action::Rename));
    assert!(
        rt.find("reset-binding-rename").is_some(),
        "the file names Rename now, so it can be reset"
    );

    click(&mut rt, "reset-binding-rename");
    assert_eq!(
        rt.state.keybinds.chord(Action::Rename),
        Keybinds::defaults().chord(Action::Rename),
        "resetting brings the default back"
    );
    assert!(
        rt.find("reset-binding-rename").is_none(),
        "the undo goes with the line"
    );
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("rename"),
        "the line is out of the file: {text}"
    );
    assert!(!saved_keybinds(&dir).is_custom(Action::Rename));
}

/// A setting the config file names can be reset, which takes its key out rather than writing
/// the default down; a setting the file doesn't name has no Reset at all.
#[test]
fn resetting_a_setting_takes_its_key_out_of_the_config() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    assert!(
        rt.find("reset-hidden").is_none(),
        "there is nothing to reset until the file names the setting"
    );

    click(&mut rt, "setting-hidden");
    click(&mut rt, "setting-inspector");
    assert!(
        rt.find("reset-hidden").is_some(),
        "the switch writes the setting into the file"
    );

    click(&mut rt, "reset-hidden");
    assert!(!rt.state.show_hidden, "resetting brings the default back");
    assert!(
        rt.find("reset-hidden").is_none(),
        "the Reset goes with the key"
    );
    let config = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(!config.contains("show-hidden"), "{config}");
    assert!(
        config.contains("inspector #false"),
        "the setting left alone stays: {config}"
    );
}
