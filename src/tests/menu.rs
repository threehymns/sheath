use super::helpers::*;
use crate::app::SheetKind;
use rune::{Key, KeyEvent, Mods};
use std::fs;

#[test]
fn right_clicking_a_row_opens_the_menu_and_rename_opens_a_focused_sheet() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    right_click(&mut rt, "f:notes.md");
    assert_eq!(rt.state.selected.as_deref(), Some("f:notes.md"));
    assert!(rt.state.menu.is_some(), "the row menu opens at the press");
    assert!(rt.find("row-menu-item0").is_some());

    click(&mut rt, "row-menu-item3");
    assert!(rt.state.menu.is_none(), "picking an item closes the menu");
    assert_eq!(
        rt.state.sheet.as_ref().map(|s| s.kind),
        Some(SheetKind::Rename)
    );
    assert!(rt.find("rename-dialog").is_some());

    rt.key(KeyEvent {
        key: Key::Text("renamed".into()),
        mods: Mods::default(),
    });
    rt.frame(0.0);
    rt.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    rt.frame(0.1);
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
    assert!(rt.find("row-menu-item4").is_some(), "Move is in the menu");
    assert!(rt.find("row-menu-item6").is_some(), "Trash is in the menu");

    click(&mut rt, "row-menu-item6");
    assert_eq!(
        rt.state.sheet.as_ref().map(|s| s.kind),
        Some(SheetKind::Trash)
    );
    assert!(rt.find("trash-dialog").is_some());
}

#[test]
fn right_clicking_any_column_opens_the_menu_for_that_row() {
    let dir = fixture();
    let (size, date) = {
        let mut rt = runtime(app(&dir));
        rt.frame(0.0);
        (
            rt.find("size").expect("the size header is on screen").0,
            rt.find("modified").expect("the date header is on screen").0,
        )
    };
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
fn the_row_menu_offers_to_hide_an_entry_a_rule_revealed() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "entry-rule \"secret\" {{\n    match path=\"{}\"\n    hidden #false\n}}\n",
            dir.path().join(".secret").display()
        ),
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(
        names(&rt.state).contains(&".secret"),
        "the rule keeps the dot file on screen"
    );

    right_click(&mut rt, "f:.secret");
    let elements = rt.elements();
    let labels: Vec<&str> = elements
        .iter()
        .filter_map(|element| element.text.as_deref())
        .collect();
    assert!(
        labels.contains(&"Always Hide This Entry"),
        "the menu reads the row as shown: {labels:?}"
    );
}

#[test]
fn the_row_menu_copies_and_duplicates_the_clicked_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    right_click(&mut rt, "f:notes.md");
    click(&mut rt, "row-menu-item8");
    assert!(rt.state.clipboard.is_some(), "Copy fills the clipboard");

    right_click(&mut rt, "f:notes.md");
    click(&mut rt, "row-menu-item11");
    rt.frame(0.1);
    assert!(
        dir.path().join("notes copy.md").exists(),
        "Duplicate copies the row"
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
fn right_clicking_past_the_last_row_opens_the_folders_own_menu() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let ([x, y, w, h], _) = rt.presented("files-table").expect("the list is on screen");
    let (at_x, at_y) = (x + w * 0.5, y + h - 24.0);
    rt.secondary_down(at_x, at_y);
    rt.secondary_up(at_x, at_y);
    rt.frame(0.1);

    assert!(rt.state.empty_menu.is_some(), "the folder's menu opens");
    assert!(
        rt.state.selected.is_none(),
        "the press left the selection alone"
    );
    assert!(
        rt.find("empty-menu-item0").is_some(),
        "New Folder is offered"
    );

    click(&mut rt, "empty-menu-item0");
    assert!(rt.state.empty_menu.is_none(), "picking an item closes it");
    assert!(dir.path().join("New Folder").is_dir());
}
