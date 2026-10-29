//! The rename, move and trash sheets and the operations they apply.

use super::helpers::*;
use crate::app::SheetKind;
use crate::keybinds::Action;
use rune::{Key, KeyEvent, Mods, Tx};
use std::fs;

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
