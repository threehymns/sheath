use std::fs;

use rune::text::TextSystem;
use rune::{Env, Key, KeyEvent, Mods, PhysicalInput, PhysicalKeyCode, Runtime, ShortcutError, Tx};
use rune_kit::SortDir;
use tempfile::TempDir;

use crate::app::{App, Preview, SettingsTab, SheetKind};
use crate::keybinds::{Action, Keybinds};
use crate::settings::Settings;
use crate::ui;

fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("alpha")).unwrap();
    fs::create_dir(dir.path().join(".secret")).unwrap();
    fs::write(dir.path().join("file2.txt"), "two").unwrap();
    fs::write(dir.path().join("file10.txt"), "ten!").unwrap();
    fs::write(dir.path().join("notes.md"), "# Notes\n\nHello preview.\n").unwrap();
    dir
}

fn app(dir: &TempDir) -> App {
    let mut app = App::open(dir.path().to_path_buf());
    // The defaults, not whatever the machine running the tests has saved.
    app.keybinds = Keybinds::defaults();
    app.recorders = app.keybinds.recorders();
    app.config = Settings::defaults();
    app
}

/// An app whose keybinds and settings save into `dir`, so a test never touches the real config.
fn app_in_config_dir(dir: &TempDir) -> (App, std::path::PathBuf) {
    let path = dir.path().join("keybinds.kdl");
    let mut app = app(dir);
    app.keybinds = Keybinds::at(path.clone());
    app.recorders = app.keybinds.recorders();
    app.config = Settings::at(dir.path().join("config.kdl"));
    (app, path)
}

fn names(app: &App) -> Vec<&str> {
    app.visible
        .iter()
        .map(|&i| app.entries[i].name.as_str())
        .collect()
}

#[test]
fn folders_sort_first_and_names_sort_naturally() {
    let dir = fixture();
    let app = app(&dir);
    assert_eq!(
        names(&app),
        ["alpha", "file2.txt", "file10.txt", "notes.md"]
    );
}

#[test]
fn hidden_entries_stay_out_until_asked_for() {
    let dir = fixture();
    let mut app = app(&dir);
    assert!(!names(&app).contains(&".secret"));
    app.toggle_hidden();
    assert!(names(&app).contains(&".secret"));
    assert_eq!(app.phrase_for_count(), "5 items");
}

#[test]
fn searching_filters_the_visible_rows() {
    let dir = fixture();
    let mut app = app(&dir);
    app.search = rune::TextEdit::new("file");
    app.apply_filter();
    assert_eq!(names(&app), ["file2.txt", "file10.txt"]);
    assert_eq!(app.phrase_for_count(), "2 of 4 items");
}

#[test]
fn sorting_by_size_keeps_folders_on_top() {
    let dir = fixture();
    let mut app = app(&dir);
    app.sort = ("size".to_owned(), SortDir::Descending);
    app.sort_entries();
    app.apply_filter();
    assert_eq!(
        names(&app),
        ["alpha", "notes.md", "file10.txt", "file2.txt"]
    );
}

#[test]
fn navigation_has_a_history() {
    let dir = fixture();
    let mut app = app(&dir);
    let start = app.cwd.clone();
    let alpha = dir.path().join("alpha");

    app.navigate(alpha.clone());
    assert_eq!(app.cwd, alpha);
    assert!(app.can_back());
    assert!(!app.can_forward());

    app.back();
    assert_eq!(app.cwd, start);
    assert!(app.can_forward());

    app.forward();
    assert_eq!(app.cwd, alpha);
}

#[test]
fn selecting_a_text_file_fills_the_preview() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:notes.md");
    assert_eq!(app.selected.as_deref(), Some("f:notes.md"));
    let Some(Preview::Text(body)) = &app.preview else {
        panic!("expected a text preview")
    };
    assert!(body.contains("Hello preview."));

    app.select_id("f:alpha");
    assert!(matches!(app.preview, Some(Preview::Folder(0))));
}

#[test]
fn new_folder_picks_a_free_name_and_selects_it() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();

    app.new_folder(&mut tx);
    assert!(dir.path().join("New Folder").is_dir());
    assert_eq!(app.selected.as_deref(), Some("f:New Folder"));

    app.new_folder(&mut tx);
    assert!(dir.path().join("New Folder 2").is_dir());
    assert_eq!(app.selected.as_deref(), Some("f:New Folder 2"));
    assert!(names(&app).contains(&"New Folder 2"));
}

fn runtime(app: App) -> Runtime<App> {
    let text = TextSystem::new(rune_theme_opencode::FONTS, rune_theme_opencode::FAMILIES);
    let mut rt = Runtime::with_text(app, ui::view, (1180.0, 760.0), Env::new(true, 2.0), text);
    rt.on_key = Some(crate::app::on_key);
    rt
}

#[test]
fn clicking_selects_a_row_and_double_clicking_enters_the_folder() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x, y) = rt.find("f:alpha").expect("the alpha row is on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert_eq!(rt.state.selected.as_deref(), Some("f:alpha"));
    assert_eq!(rt.state.cwd, dir.path());

    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert_eq!(rt.state.cwd, dir.path().join("alpha"));
}

#[test]
fn arrow_keys_walk_the_rows_when_nothing_has_focus() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    rt.key(KeyEvent {
        key: Key::Down,
        mods: Mods::default(),
    });
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.selected.as_deref(), Some("f:alpha"));

    rt.state.navigate(dir.path().join("alpha"));
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Backspace,
        mods: Mods::default(),
    });
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.cwd, dir.path());
}

#[test]
fn renaming_moves_the_file_and_selects_the_new_name() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:notes.md");
    app.begin_rename();
    app.sheet.as_mut().unwrap().edit.replace_text("renamed.md");
    app.submit_sheet(&mut Tx::default());

    assert!(app.sheet.is_none());
    assert!(!dir.path().join("notes.md").exists());
    assert!(dir.path().join("renamed.md").exists());
    assert_eq!(app.selected.as_deref(), Some("f:renamed.md"));
}

#[test]
fn renaming_onto_an_existing_name_keeps_the_sheet_open_with_an_error() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:notes.md");
    app.begin_rename();
    app.sheet.as_mut().unwrap().edit.replace_text("file2.txt");
    app.submit_sheet(&mut Tx::default());

    let sheet = app.sheet.as_ref().expect("the sheet stays open");
    assert!(
        sheet
            .error
            .as_deref()
            .is_some_and(|e| e.contains("already exists"))
    );
    assert!(dir.path().join("notes.md").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("file2.txt")).unwrap(),
        "two"
    );
}

#[test]
fn moving_puts_the_file_in_the_destination_folder() {
    let dir = fixture();
    let mut app = app(&dir);
    let destination = dir.path().join("moved");
    fs::create_dir(&destination).unwrap();

    app.select_id("f:file2.txt");
    app.begin_move();
    app.sheet
        .as_mut()
        .unwrap()
        .edit
        .replace_text(destination.display().to_string());
    app.submit_sheet(&mut Tx::default());

    assert!(app.sheet.is_none());
    assert!(!dir.path().join("file2.txt").exists());
    assert_eq!(
        fs::read_to_string(destination.join("file2.txt")).unwrap(),
        "two"
    );
    assert!(app.selected.is_none());
}

#[test]
fn moving_a_folder_into_itself_is_rejected() {
    let dir = fixture();
    let mut app = app(&dir);
    let inside = dir.path().join("alpha");
    fs::create_dir(inside.join("child")).unwrap();

    app.select_id("f:alpha");
    app.begin_move();
    app.sheet
        .as_mut()
        .unwrap()
        .edit
        .replace_text(inside.join("child").display().to_string());
    app.submit_sheet(&mut Tx::default());

    let sheet = app.sheet.as_ref().expect("the sheet stays open");
    assert!(sheet.error.as_deref().is_some_and(|e| e.contains("itself")));
    assert!(dir.path().join("alpha").is_dir());
}

#[test]
fn the_trash_sheet_asks_before_deleting_and_closes_without_touching_the_file() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:file10.txt");
    app.begin_trash();
    let sheet = app.sheet.as_ref().expect("trash confirmation opens");
    assert_eq!(sheet.name, "file10.txt");
    app.close_sheet();
    assert!(app.sheet.is_none());
    assert!(dir.path().join("file10.txt").exists());
}

#[test]
fn the_toolbar_button_creates_and_selects_a_folder() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    let (x, y) = rt
        .find("new-folder")
        .expect("the New Folder button is in the toolbar");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert!(dir.path().join("New Folder").is_dir());
    assert_eq!(rt.state.selected.as_deref(), Some("f:New Folder"));
}

fn right_click(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).expect("the row is on screen");
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(0.1);
}

fn click(rt: &mut Runtime<App>, key: &str) {
    let (x, y) = rt.find(key).expect("the control is on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
}

#[test]
fn right_clicking_a_row_opens_the_menu_and_rename_opens_a_focused_sheet() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    right_click(&mut rt, "f:notes.md");
    assert_eq!(rt.state.selected.as_deref(), Some("f:notes.md"));
    assert!(rt.state.menu.is_some(), "the row menu opens at the press");
    assert!(rt.find("row-menu-item0").is_some());

    click(&mut rt, "row-menu-item0");
    assert!(rt.state.menu.is_none(), "picking an item closes the menu");
    assert_eq!(
        rt.state.sheet.as_ref().map(|s| s.kind),
        Some(SheetKind::Rename)
    );
    assert!(rt.find("rename-dialog").is_some());

    rt.key(KeyEvent {
        key: Key::Text("renamed.md".into()),
        mods: Mods::default(),
    });
    rt.frame(0.0);
    rt.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    rt.frame(0.1);

    assert!(rt.state.sheet.is_none());
    assert!(!dir.path().join("notes.md").exists());
    assert!(dir.path().join("renamed.md").exists());
    assert_eq!(rt.state.selected.as_deref(), Some("f:renamed.md"));
}

#[test]
fn the_row_menu_offers_move_and_trash_for_the_clicked_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    right_click(&mut rt, "f:file2.txt");
    assert!(rt.find("row-menu-item1").is_some(), "Move is in the menu");
    assert!(rt.find("row-menu-item3").is_some(), "Trash is in the menu");

    click(&mut rt, "row-menu-item3");
    assert_eq!(
        rt.state.sheet.as_ref().map(|s| s.kind),
        Some(SheetKind::Trash)
    );
    assert!(rt.find("trash-dialog").is_some());
}

#[test]
fn right_clicking_any_column_opens_the_menu_for_that_row() {
    let dir = fixture();
    // Where the size and date columns sit, from their headers.
    let (size, date) = {
        let mut rt = runtime(app(&dir));
        rt.frame(0.0);
        (
            rt.find("size").expect("the size header is on screen").0,
            rt.find("modified").expect("the date header is on screen").0,
        )
    };
    // The name, the padding either side of the column rules, the size and the date.
    for x in [300.0, size - 46.5, size, date - 79.5, date] {
        let mut rt = runtime(app(&dir));
        rt.frame(0.0);
        let (_, y) = rt.find("f:file10.txt").expect("the row is on screen");
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        rt.frame(0.1);

        assert!(rt.state.menu.is_some(), "the menu opens at x = {x}");
        assert_eq!(rt.state.selected.as_deref(), Some("f:file10.txt"));
    }
}

#[test]
fn escape_and_an_outside_press_close_the_row_menu() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    right_click(&mut rt, "f:notes.md");
    assert!(rt.state.menu.is_some());
    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.menu.is_none());

    right_click(&mut rt, "f:notes.md");
    assert!(rt.state.menu.is_some());
    click(&mut rt, "path-bar");
    assert!(rt.state.menu.is_none());
}

#[test]
fn escape_dismisses_the_trash_confirmation() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:file2.txt");
    app.begin_trash();
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(rt.find("trash-dialog").is_some());
    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.sheet.is_none());
    assert!(dir.path().join("file2.txt").exists());
}

#[test]
fn the_inspector_builds_a_scrollable_preview() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:notes.md");
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(rt.find("preview-scroll").is_some());
    assert!(rt.find("files-table").is_some());
}

/// The event a binding's chord arrives as, with the key the shortcut names.
fn event_for(app: &App, action: Action, key: Key) -> KeyEvent {
    let chord = app.keybinds.chord(action).expect("the action is bound");
    KeyEvent {
        key,
        mods: chord.modifiers(),
    }
}

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

#[test]
fn the_settings_panel_has_a_sidebar_of_categories() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    assert_eq!(rt.state.settings, Some(SettingsTab::General));
    assert!(rt.find("settings").is_some());
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

    click(&mut rt, "reset-keybinds");
    assert_eq!(
        rt.state.keybinds.chord(Action::Rename),
        Keybinds::defaults().chord(Action::Rename)
    );

    click(&mut rt, "settings-tab-about");
    assert_eq!(rt.state.settings, Some(SettingsTab::About));
    assert!(rt.find("copy-keybinds-path").is_some());

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
fn the_settings_panel_covers_the_browser() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    // The row is still laid out under the panel; a press there lands on the panel instead.
    let (x, y) = rt.find("f:alpha").expect("the row is under the panel");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert!(rt.state.selected.is_none(), "no row was clicked");
    assert_eq!(rt.state.settings, Some(SettingsTab::General));
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

    let config = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(config.contains("show-hidden #true"), "{config}");
    assert!(config.contains("inspector #false"), "{config}");
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

    let saved = Settings::at(path);
    assert_eq!(saved.error, None);
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

    let saved = Settings::at(path);
    assert!(
        saved
            .error
            .as_deref()
            .is_some_and(|e| e.contains("Couldn’t read")),
        "{:?}",
        saved.error
    );
    assert!(!saved.show_hidden);
    assert!(saved.inspector);
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
    let saved = Keybinds::at(path);
    assert_eq!(saved.chord(Action::Rename), Some(&f5), "the file has it");
}

fn command_bar_event(app: &App, shifted: bool) -> KeyEvent {
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

#[test]
fn ctrl_p_and_ctrl_shift_p_open_the_command_bar() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    let open = command_bar_event(&rt.state, false);
    rt.key(open.clone());
    rt.frame(0.1);
    assert!(rt.state.palette.is_some(), "Ctrl+P opens it");
    assert!(rt.find("command-bar").is_some());
    assert!(
        rt.find("command-rename").is_some(),
        "the registry is listed"
    );

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.palette.is_none(), "Escape closes it");

    rt.key(command_bar_event(&rt.state, true));
    rt.frame(0.1);
    assert!(rt.state.palette.is_some(), "Ctrl+Shift+P opens it too");

    rt.key(open);
    rt.frame(0.1);
    assert!(rt.state.palette.is_none(), "its chord closes it again");
}

#[test]
fn the_command_bar_filters_and_runs_a_command() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.select_id("f:notes.md");

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Text("rena".into()),
        mods: Mods::default(),
    });
    rt.frame(0.1);

    assert!(rt.find("command-rename").is_some());
    assert!(
        rt.find("command-refresh").is_none(),
        "the query filters the other commands out"
    );

    rt.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.palette.is_none());
    assert_eq!(
        rt.state.sheet.as_ref().map(|s| s.kind),
        Some(SheetKind::Rename),
        "Return ran the filtered command"
    );
}

#[test]
fn the_command_bar_runs_a_command_picked_with_the_pointer() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    click(&mut rt, "command-new-folder");

    assert!(rt.state.palette.is_none(), "picking one closes the bar");
    assert!(dir.path().join("New Folder").is_dir());
}

#[test]
fn the_command_bar_moves_the_selection_with_the_arrows() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    assert_eq!(rt.state.palette.as_ref().map(|p| p.selected), Some(0));

    rt.key(KeyEvent {
        key: Key::Down,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert_eq!(rt.state.palette.as_ref().map(|p| p.selected), Some(1));
    rt.key(KeyEvent {
        key: Key::Up,
        mods: Mods::default(),
    });
    rt.frame(0.1);

    let (x, y) = rt
        .find("command-open-settings")
        .expect("Settings is listed");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert_eq!(rt.state.settings, Some(SettingsTab::General));
}

#[test]
fn the_command_bar_leaves_commands_that_need_a_selection_alone() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Text("trash".into()),
        mods: Mods::default(),
    });
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    rt.frame(0.1);

    assert!(rt.state.sheet.is_none(), "nothing is selected to trash");
    assert!(rt.state.palette.is_none());
}

/// Clicks `key` while `mods` are held, as a Shift- or Cmd-click.
fn click_with(rt: &mut Runtime<App>, key: &str, mods: Mods) {
    rt.physical_input(PhysicalInput::Modifiers(mods));
    click(rt, key);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(0.1);
}

fn shift() -> Mods {
    Mods {
        shift: true,
        ..Mods::default()
    }
}

fn command() -> Mods {
    Mods {
        cmd: true,
        ..Mods::default()
    }
}

#[test]
fn shift_clicking_extends_the_selection_from_the_last_click() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    assert_eq!(rt.state.selected_ids(), ["f:file2.txt"]);

    click_with(&mut rt, "f:notes.md", shift());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file2.txt", "f:file10.txt", "f:notes.md"],
        "Shift-click selects the range between the anchor and the click"
    );
    assert_eq!(rt.state.phrase_for_count(), "3 of 4 selected");
}

#[test]
fn command_clicking_toggles_a_row_in_and_out_of_the_selection() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", command());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file2.txt", "f:notes.md"],
        "Cmd-click adds a row"
    );

    click_with(&mut rt, "f:notes.md", command());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file2.txt"],
        "Cmd-click again takes it back out"
    );
    assert_eq!(
        rt.state.selected.as_deref(),
        Some("f:file2.txt"),
        "the inspector follows a row that is still selected"
    );
}

#[test]
fn the_row_menu_keeps_a_selection_it_lands_in() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    right_click(&mut rt, "f:file10.txt");
    assert_eq!(
        rt.state.selection_len(),
        3,
        "a right-click inside the selection acts on all of it"
    );
    // A labelled item says how many rows it covers.
    assert_eq!(rt.state.selection_len(), 3);

    rt.state.close_menu();
    rt.invalidate();
    rt.frame(0.1);
    right_click(&mut rt, "f:alpha");
    assert_eq!(
        rt.state.selected_ids(),
        ["f:alpha"],
        "a right-click outside the selection selects that row alone"
    );
}

#[test]
fn moving_a_selection_moves_every_selected_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    let destination = dir.path().join("moved");
    fs::create_dir(&destination).unwrap();

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    rt.state.begin_move();
    let sheet = rt.state.sheet.as_mut().expect("the move sheet opens");
    assert_eq!(sheet.targets.len(), 3);
    sheet.edit.replace_text(destination.display().to_string());
    rt.state.submit_sheet(&mut Tx::default());

    assert!(rt.state.sheet.is_none(), "the sheet closes");
    assert!(!dir.path().join("file2.txt").exists());
    assert!(!dir.path().join("file10.txt").exists());
    assert!(!dir.path().join("notes.md").exists());
    for name in ["file2.txt", "file10.txt", "notes.md"] {
        assert!(destination.join(name).exists(), "{name} moved");
    }
    assert!(dir.path().join("alpha").is_dir(), "the folder stayed put");
}

#[test]
fn the_trash_sheet_covers_the_whole_selection() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    rt.state.begin_trash();
    rt.invalidate();
    rt.frame(0.1);

    let sheet = rt.state.sheet.as_ref().expect("the confirmation opens");
    assert_eq!(sheet.name, "3 items");
    assert_eq!(sheet.targets.len(), 3);
    assert!(rt.find("trash-dialog").is_some());
    // Nothing is deleted until the button is pressed.
    for name in ["file2.txt", "file10.txt", "notes.md"] {
        assert!(dir.path().join(name).exists());
    }
}

#[test]
fn renaming_needs_exactly_one_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    assert!(!rt.state.action_enabled(Action::Rename));
    rt.state.begin_rename();
    assert!(rt.state.sheet.is_none(), "no rename sheet for a selection");

    let f2 = KeyEvent {
        key: Key::Function(2),
        mods: Mods::default(),
    };
    rt.key(f2);
    rt.frame(0.1);
    assert!(rt.state.sheet.is_none(), "F2 leaves a selection alone");
    assert_eq!(rt.state.selection_len(), 3);

    click(&mut rt, "f:file2.txt");
    assert!(rt.state.action_enabled(Action::Rename));
}

#[test]
fn the_inspector_summarises_a_selection_of_several_rows() {
    let dir = fixture();
    // The table owns the selection, so select the way a click does.
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());

    assert!(rt.find("selection-summary").is_some());
    assert!(
        rt.find("preview-scroll").is_none(),
        "no single file preview for several rows"
    );
    // The two smaller files and the Markdown note, which the fixture writes with 24 bytes.
    assert_eq!(rt.state.selection_size(), 3 + 4 + 24);
    assert_eq!(rt.state.selection_folders(), 0);
}

#[test]
fn escape_takes_a_selection_back_to_one_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    assert_eq!(rt.state.selection_len(), 3);

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert_eq!(rt.state.selection_len(), 1, "Escape narrows the selection");
    assert!(rt.state.selected.is_some());
}

#[test]
fn refreshing_keeps_a_selection_of_several_rows() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    rt.state.reload();
    rt.invalidate();
    rt.frame(0.1);
    assert_eq!(rt.state.selection_len(), 3, "a refresh keeps the selection");
}

fn control_shift() -> Mods {
    Mods {
        ctrl: true,
        shift: true,
        ..Mods::default()
    }
}

#[test]
fn ctrl_shift_clicking_keeps_the_selection_and_adds_a_range() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:file10.txt", shift());
    // The anchor is file2, so the range runs file2..file10.
    assert_eq!(rt.state.selected_ids(), ["f:file2.txt", "f:file10.txt"]);

    click_with(&mut rt, "f:notes.md", control_shift());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file2.txt", "f:file10.txt", "f:notes.md"],
        "Ctrl+Shift+click adds the range and keeps what was selected"
    );
}

#[test]
fn ctrl_shift_clicking_after_a_far_click_keeps_both_groups() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:notes.md");
    click_with(&mut rt, "f:alpha", control_shift());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:alpha", "f:file2.txt", "f:file10.txt", "f:notes.md"],
        "the range from the anchor joins the row that was already selected"
    );
}

#[test]
fn command_a_selects_every_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    rt.key(KeyEvent {
        key: Key::Command('a'),
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert_eq!(rt.state.selection_len(), 4);
    assert_eq!(rt.state.phrase_for_count(), "4 of 4 selected");
}

#[test]
fn shift_arrow_extends_the_selection() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    for _ in 0..2 {
        rt.key(KeyEvent {
            key: Key::Down,
            mods: shift(),
        });
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file2.txt", "f:file10.txt", "f:notes.md"],
        "Shift+Down grows the range from the anchor"
    );

    rt.key(KeyEvent {
        key: Key::Down,
        mods: Mods::default(),
    });
    rt.frame(1.0 / 60.0);
    assert_eq!(
        rt.state.selection_len(),
        1,
        "a plain arrow collapses back to one row"
    );
}

/// Drags a box from the list's empty space up to a row, holding `mods`.
fn drag_box(rt: &mut Runtime<App>, to: &str, mods: Mods) {
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

#[test]
fn dragging_a_box_selects_the_rows_it_covers() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:alpha");
    drag_box(&mut rt, "f:file2.txt", Mods::default());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file2.txt", "f:file10.txt", "f:notes.md"],
        "a plain box replaces the selection with the rows it swept"
    );
    assert!(rt.state.marquee.is_none(), "the box ends with the button");
}

#[test]
fn a_shift_drag_adds_the_box_to_the_selection() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:alpha");
    drag_box(&mut rt, "f:file2.txt", shift());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:alpha", "f:file2.txt", "f:file10.txt", "f:notes.md"],
        "Shift keeps what was selected and adds the box"
    );
}

#[test]
fn a_ctrl_drag_toggles_the_rows_in_the_box() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    drag_box(&mut rt, "f:file2.txt", command());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file10.txt", "f:notes.md"],
        "Cmd/Ctrl toggles the rows the box covers"
    );
}

#[test]
fn a_ctrl_shift_drag_keeps_the_selection_and_adds_the_box() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:alpha");
    drag_box(&mut rt, "f:file2.txt", control_shift());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:alpha", "f:file2.txt", "f:file10.txt", "f:notes.md"],
        "Ctrl+Shift+drag adds the box and keeps the rest"
    );
}

#[test]
fn a_header_click_sorts_and_clicking_again_reverses_it() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "size");
    assert_eq!(
        names(&rt.state),
        ["alpha", "file2.txt", "file10.txt", "notes.md"]
    );
    click(&mut rt, "size");
    assert_eq!(
        names(&rt.state),
        ["alpha", "notes.md", "file10.txt", "file2.txt"]
    );
    assert_eq!(
        rt.state.sort_column(),
        ("size", SortDir::Descending),
        "the header shows the sort it set"
    );
}

#[test]
fn column_widths_clamp_to_a_usable_range() {
    let mut app = app(&fixture());
    assert_eq!(app.size_width, 92.0);
    app.resize_column("size", 10.0);
    assert_eq!(app.size_width, 64.0, "too narrow is clamped");
    app.resize_column("size", 900.0);
    assert_eq!(app.size_width, 320.0, "too wide is clamped");
    app.reset_column("size");
    assert_eq!(app.size_width, 92.0);
    app.resize_column("modified", 240.0);
    assert_eq!(app.date_width, 240.0);
}

#[test]
fn a_press_without_moving_is_still_a_click() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x, y) = rt.find("f:file2.txt").expect("the row is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.05);
    assert!(
        rt.find("marquee").is_none(),
        "no box is drawn before the pointer moves"
    );
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert_eq!(rt.state.selected_ids(), ["f:file2.txt"]);
    assert!(rt.state.marquee.is_none());
}

#[test]
fn a_drag_draws_the_box_while_the_button_is_down() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x0, y0) = rt.find("files-table").expect("the list is on screen");
    let (_, y1) = rt.find("f:notes.md").expect("the row is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x0 + 20.0, y1);
    rt.frame(0.05);
    assert!(
        rt.find("marquee").is_some(),
        "the box is drawn while dragging"
    );
    rt.pointer_up(x0 + 20.0, y1);
    rt.frame(0.1);
    assert!(rt.find("marquee").is_none(), "and goes with the button");
}

#[test]
fn dragging_a_header_edge_resizes_its_column() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x, y) = rt
        .find("size/resize")
        .expect("the resize handle is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.05);
    rt.pointer_move(x + 40.0, y);
    rt.frame(0.05);
    rt.pointer_up(x + 40.0, y);
    rt.frame(0.1);
    assert!(
        rt.state.size_width > 100.0,
        "the size column grew: {}",
        rt.state.size_width
    );
}

/// Drags one row's press onto another row with the pointer, as a file drag does.
fn drag_row(rt: &mut Runtime<App>, from: &str, to: &str) {
    let (x0, y0) = rt.find(from).expect("the first row is on screen");
    let (x1, y1) = rt.find(to).expect("the second row is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x1, y1);
    rt.frame(0.05);
    rt.pointer_up(x1, y1);
    rt.frame(0.1);
}

#[test]
fn dragging_a_row_onto_a_folder_moves_it() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    drag_row(&mut rt, "f:file2.txt", "f:alpha");

    assert!(
        dir.path().join("alpha").join("file2.txt").exists(),
        "the file landed in the folder it was dropped on"
    );
    assert!(!dir.path().join("file2.txt").exists());
    assert!(rt.state.error.is_none());
}

#[test]
fn dragging_a_selection_onto_a_folder_moves_all_of_it() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    // The press lands on a row that is already selected, so the whole selection travels.
    drag_row(&mut rt, "f:file10.txt", "f:alpha");

    for name in ["file2.txt", "file10.txt", "notes.md"] {
        assert!(
            dir.path().join("alpha").join(name).exists(),
            "{name} moved with the selection"
        );
        assert!(!dir.path().join(name).exists());
    }
}

#[test]
fn dropping_a_row_on_a_file_or_empty_space_moves_nothing() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    drag_row(&mut rt, "f:file2.txt", "f:notes.md");
    assert!(dir.path().join("file2.txt").exists());

    let (x0, y0) = rt.find("f:file2.txt").expect("the row is on screen");
    let (x1, y1) = rt.find("files-table").expect("the list is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x1, y1);
    rt.frame(0.05);
    rt.pointer_up(x1, y1);
    rt.frame(0.1);
    assert!(dir.path().join("file2.txt").exists());
    assert!(rt.state.file_drag.is_none());
}

#[test]
fn a_row_drag_shows_a_badge_and_no_selection_box() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x0, y0) = rt.find("f:file2.txt").expect("the row is on screen");
    let (x1, y1) = rt.find("f:alpha").expect("the folder row is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x1, y1);
    rt.frame(0.05);

    assert!(
        rt.find("drag-badge").is_some(),
        "the drag says what it carries"
    );
    assert!(rt.find("marquee").is_none(), "a row drag draws no box");
    assert_eq!(rt.state.drop_row(), Some(0), "the folder row highlights");

    rt.pointer_up(x1, y1);
    rt.frame(0.1);
    assert!(rt.find("drag-badge").is_none());
}

#[test]
fn a_drag_onto_a_sidebar_place_moves_the_selection_there() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    let destination = dir.path().join("moved");
    fs::create_dir(&destination).unwrap();
    // A place stands in for the sidebar rows, below them where nothing else is drawn.
    let rect = [20.0, 620.0, 180.0, 40.0];
    rt.state
        .places_at
        .insert("test-place", (rect, destination.clone()));

    click(&mut rt, "f:file2.txt");
    let (x0, y0) = rt.find("f:file2.txt").expect("the row is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(rect[0] + 40.0, rect[1] + 20.0);
    rt.frame(0.05);
    assert_eq!(rt.state.drop_place(), Some("test-place"));
    rt.pointer_up(rect[0] + 40.0, rect[1] + 20.0);
    rt.frame(0.1);

    assert!(destination.join("file2.txt").exists());
    assert!(!dir.path().join("file2.txt").exists());
}

#[test]
fn a_dropped_selection_that_collides_is_refused_and_stays_put() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    fs::write(dir.path().join("alpha").join("file2.txt"), "already here").unwrap();

    click(&mut rt, "f:file2.txt");
    drag_row(&mut rt, "f:file2.txt", "f:alpha");

    assert!(rt.state.error.is_some(), "the refusal is reported");
    assert!(dir.path().join("file2.txt").exists(), "nothing moved");
    assert_eq!(
        fs::read_to_string(dir.path().join("alpha").join("file2.txt")).unwrap(),
        "already here"
    );
}

#[test]
fn copy_and_paste_puts_a_copy_beside_the_file() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:notes.md");

    app.run_action(Action::Copy, &mut tx);
    assert_eq!(
        fs::read_to_string(dir.path().join("notes.md")).unwrap(),
        "# Notes\n\nHello preview.\n",
        "copying leaves the file alone"
    );
    app.paste();

    assert_eq!(
        fs::read_to_string(dir.path().join("notes copy.md")).unwrap(),
        "# Notes\n\nHello preview.\n"
    );
    assert_eq!(
        app.selected.as_deref(),
        Some("f:notes copy.md"),
        "the copy is selected"
    );

    app.paste();
    assert!(
        dir.path().join("notes copy 2.md").exists(),
        "a second paste numbers itself"
    );
}

#[test]
fn cut_and_paste_moves_the_file_into_the_folder_on_screen() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:file2.txt");
    app.run_action(Action::Cut, &mut tx);
    assert!(
        app.is_cut(&dir.path().join("file2.txt")),
        "cut rows are dimmed"
    );

    app.navigate(dir.path().join("alpha"));
    app.paste();

    assert!(dir.path().join("alpha").join("file2.txt").exists());
    assert!(!dir.path().join("file2.txt").exists());
    assert!(app.clipboard.is_none(), "a cut is pasted once");
    assert_eq!(app.selected.as_deref(), Some("f:file2.txt"));
}

#[test]
fn duplicate_copies_a_folder_and_everything_in_it() {
    let dir = fixture();
    let mut app = app(&dir);
    fs::create_dir(dir.path().join("alpha").join("inner")).unwrap();
    fs::write(dir.path().join("alpha").join("inner").join("a.txt"), "a").unwrap();
    let mut tx = Tx::default();

    app.select_id("f:alpha");
    app.run_action(Action::Duplicate, &mut tx);

    assert!(dir.path().join("alpha copy").is_dir());
    assert_eq!(
        fs::read_to_string(dir.path().join("alpha copy").join("inner").join("a.txt")).unwrap(),
        "a",
        "the folder's contents came with it"
    );
    assert!(dir.path().join("alpha").is_dir());
}

#[test]
fn copying_a_selection_copies_every_row() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:file2.txt");
    app.selection.insert("f:file10.txt".into());

    app.run_action(Action::Copy, &mut tx);
    app.paste();

    assert!(dir.path().join("file2 copy.txt").exists());
    assert!(dir.path().join("file10 copy.txt").exists());
    assert_eq!(app.selection_len(), 2, "both copies are selected");
}

#[test]
fn typing_a_name_jumps_to_the_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    rt.key(KeyEvent {
        key: Key::Text("n".into()),
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert_eq!(rt.state.selected.as_deref(), Some("f:notes.md"));

    // Typing another letter narrows the same search: "no" still lands on notes.md.
    rt.key(KeyEvent {
        key: Key::Text("o".into()),
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert_eq!(rt.state.selected.as_deref(), Some("f:notes.md"));

    // A fresh search starts over, and repeating the letter cycles through its matches.
    rt.state.clear_typeahead();
    for expected in ["f:file2.txt", "f:file10.txt"] {
        rt.key(KeyEvent {
            key: Key::Text("f".into()),
            mods: Mods::default(),
        });
        rt.frame(0.1);
        assert_eq!(rt.state.selected.as_deref(), Some(expected));
    }
}

#[test]
fn there_is_no_paste_until_something_is_copied() {
    let dir = fixture();
    let mut app = app(&dir);
    assert!(!app.action_enabled(Action::Paste));
    app.select_id("f:file2.txt");
    app.copy_selection(false);
    assert!(app.action_enabled(Action::Paste));

    // Pasting into the same folder would collide with itself, so a copy takes a new name.
    app.paste();
    assert!(dir.path().join("file2 copy.txt").exists());
}

#[test]
fn the_delete_key_asks_before_trashing() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:file2.txt");
    let delete = KeyEvent {
        key: Key::Delete,
        mods: Mods::default(),
    };
    assert!(crate::app::on_key(&mut app, &delete, &mut tx));
    assert_eq!(app.sheet.as_ref().map(|s| s.kind), Some(SheetKind::Trash));
    assert_eq!(
        app.sheet.as_ref().map(|s| s.name.clone()),
        Some("file2.txt".into())
    );
    assert!(dir.path().join("file2.txt").exists());
}

#[test]
fn a_click_inside_a_selection_narrows_it_to_the_clicked_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());
    assert_eq!(rt.state.selection_len(), 3);

    click(&mut rt, "f:file10.txt");
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file10.txt"],
        "a click without a drag narrows a selection to its row"
    );
}

#[test]
fn the_command_bar_rows_keep_their_padding_from_the_edges() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.state.open_palette();
    rt.invalidate();
    rt.frame(0.1);

    let ([bar_x, _, bar_w, _], _) = rt.presented("command-bar").expect("the bar is drawn");
    let (row_x, row_w) = {
        let ([x, _, w, _], _) = rt
            .presented("command-rename")
            .expect("a command row is drawn");
        (x, w)
    };
    assert!(row_x >= bar_x + 5.0, "the row starts inside the bar");
    assert!(
        row_x + row_w <= bar_x + bar_w - 5.0,
        "and stops short of the far edge instead of clipping"
    );
}
