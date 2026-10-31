//! The sidebar place menu a right-click opens: its actions, and the rename field.

use super::helpers::*;
use crate::app::SheetKind;
use crate::places::Places;
use rune::{Key, KeyEvent, Mods};
use std::fs;

#[test]
fn right_clicking_a_place_offers_a_menu_and_removing_hides_its_row() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let second_id = format!("folder:{}", second.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[
            (alpha_id.clone(), alpha.display().to_string()),
            (second_id.clone(), second.display().to_string()),
        ],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let key = format!("place-{alpha_id}");
    right_click(&mut rt, &key);
    assert!(rt.state.place_menu.is_some(), "the place menu opens");
    assert!(rt.find("place-menu-item0").is_some(), "Open is offered");
    assert!(
        rt.find("place-menu-item1").is_some(),
        "Show in Folder is offered"
    );
    assert!(rt.find("place-menu-item3").is_some(), "Rename is offered");
    assert!(
        rt.find("place-menu-item4").is_some(),
        "Rename on Disk is offered"
    );
    assert!(rt.find("place-menu-item5").is_some(), "Remove is offered");

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.place_menu.is_none(), "Escape closes the menu");
    assert!(rt.find(&key).is_some(), "and leaves the place alone");

    right_click(&mut rt, &key);
    click(&mut rt, "place-menu-item5");
    rt.frame(0.1);

    assert!(
        rt.state.place_menu.is_none(),
        "picking an item closes the menu"
    );
    assert!(rt.find(&key).is_none(), "the row leaves the sidebar");
    assert!(
        rt.state
            .places
            .get(&alpha_id)
            .is_some_and(|place| place.hidden),
        "the place is kept in the file, hidden"
    );
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("hidden=#true"),
        "the file marks it hidden: {text}"
    );
    assert!(
        rt.find(&format!("place-{second_id}")).is_some(),
        "the other place stays"
    );
}

#[test]
fn the_place_menu_renames_the_display_name_in_place() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[(alpha_id.clone(), alpha.display().to_string())],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let key = format!("place-{alpha_id}");
    right_click(&mut rt, &key);
    click(&mut rt, "place-menu-item3");
    rt.frame(0.1);

    assert!(rt.state.place_menu.is_none(), "the menu closes");
    let rename = rt
        .state
        .place_rename
        .as_ref()
        .expect("the row shows a field");
    assert_eq!(rename.edit.text, "alpha", "the field starts on the name");

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.place_rename.is_none(), "Escape ends the edit");
    assert_eq!(
        rt.state.places.get(&alpha_id).unwrap().name,
        "alpha",
        "and changes nothing"
    );

    right_click(&mut rt, &key);
    click(&mut rt, "place-menu-item3");
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Text("Projects".into()),
        mods: Mods::default(),
    });
    rt.frame(0.0);
    rt.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    rt.frame(0.1);

    assert!(rt.state.place_rename.is_none(), "Return ends the edit");
    assert_eq!(
        rt.state.places.get(&alpha_id).unwrap().name,
        "Projects",
        "the sidebar shows the new name"
    );
    assert!(alpha.is_dir(), "the folder keeps its own name on disk");
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("name=Projects"),
        "the file keeps the display name: {text}"
    );
}

#[test]
fn the_place_menu_renames_the_folder_on_disk_and_follows_it() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[(alpha_id.clone(), alpha.display().to_string())],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    // Stand in the folder being renamed, so the browser has to move with it.
    rt.state.navigate(alpha.clone());
    rt.frame(0.1);
    assert_eq!(rt.state.cwd.as_path(), alpha.as_path());

    right_click(&mut rt, &format!("place-{alpha_id}"));
    click(&mut rt, "place-menu-item4");
    rt.frame(0.1);

    assert_eq!(
        rt.state.sheet.as_ref().map(|s| s.kind),
        Some(SheetKind::RenamePlace)
    );
    assert!(rt.find("rename-place-dialog").is_some());
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

    let renamed = dir.path().join("renamed");
    assert!(rt.state.sheet.is_none(), "the sheet closes");
    assert!(!alpha.exists(), "the folder left its old name behind");
    assert!(renamed.is_dir(), "the folder has its new name on disk");
    assert_eq!(
        rt.state.cwd.as_path(),
        renamed.as_path(),
        "the open folder follows the rename"
    );

    let new_id = format!("folder:{}", renamed.display());
    let place = rt
        .state
        .places
        .get(&new_id)
        .expect("the place moved to the new path");
    assert_eq!(place.path, renamed);
    assert_eq!(place.name, "renamed", "a derived name follows the folder");
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains(&crate::rules::quote(&new_id)),
        "the file holds the new path: {text}"
    );
}

#[test]
fn the_place_menu_shows_a_place_in_its_folder() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[(alpha_id.clone(), alpha.display().to_string())],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    // Move away, so showing the place has to bring its folder back.
    rt.state.navigate(second.clone());
    rt.frame(0.1);
    assert_eq!(rt.state.cwd.as_path(), second.as_path());

    right_click(&mut rt, &format!("place-{alpha_id}"));
    click(&mut rt, "place-menu-item1");
    rt.frame(0.1);
    rt.frame(0.1);

    assert_eq!(
        rt.state.cwd.as_path(),
        dir.path(),
        "the folder holding the place is open"
    );
    assert_eq!(
        rt.state.selected.as_deref(),
        Some("f:alpha"),
        "the place is selected there"
    );
}
