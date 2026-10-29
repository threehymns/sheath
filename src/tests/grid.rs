//! The grid view: switching to it, clicking and walking its tiles, and remembering the choice.

use super::helpers::*;
use crate::app::{App, ViewMode};
use crate::settings::Settings;
use rune::{Key, KeyEvent, Mods};
use std::fs;

#[test]
fn the_toolbar_switches_between_the_list_and_the_grid() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    assert_eq!(rt.state.view, ViewMode::List);
    assert!(
        rt.find("header").is_some(),
        "the list has its column header"
    );

    click(&mut rt, "toggle-view");
    assert_eq!(rt.state.view, ViewMode::Grid);
    assert!(rt.find("file-grid").is_some(), "the grid is on screen");
    assert!(rt.find("header").is_none(), "the grid has no column header");
    assert!(
        rt.find("f:notes.md").is_some(),
        "the file has a tile on screen"
    );

    click(&mut rt, "toggle-view");
    assert_eq!(rt.state.view, ViewMode::List);
    assert!(rt.find("file-grid").is_none());
    assert!(rt.find("header").is_some());
}

#[test]
fn clicking_a_tile_selects_it() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-view");
    rt.frame(0.1);

    click(&mut rt, "f:notes.md");
    assert_eq!(rt.state.selected.as_deref(), Some("f:notes.md"));
    assert_eq!(rt.state.selection_len(), 1);
    assert_eq!(rt.state.cwd, dir.path(), "a single click doesn't open it");
}

#[test]
fn arrow_keys_walk_the_grid_by_tile_and_by_row() {
    let dir = long_folder(24);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-view");
    rt.frame(0.1);

    let columns = rt.state.grid_columns();
    assert!(columns > 1, "the window fits several tiles across");
    let key = |rt: &mut rune::Runtime<App>, key, mods| {
        rt.key(KeyEvent { key, mods });
        rt.frame(1.0 / 60.0);
    };

    // Clicking a tile hands the keyboard to the list, as it does in the table.
    click(&mut rt, "f:file00.txt");
    key(&mut rt, Key::Home, Mods::default());
    assert_eq!(rt.state.selected.as_deref(), Some("f:file00.txt"));

    for _ in 0..2 {
        key(&mut rt, Key::Right, Mods::default());
    }
    assert_eq!(rt.state.selected.as_deref(), Some("f:file02.txt"));

    key(&mut rt, Key::Down, Mods::default());
    let below = (2 + columns).min(23);
    assert_eq!(
        rt.state.selected.as_deref(),
        Some(format!("f:file{below:02}.txt").as_str()),
        "Down moves a grid row, not a list row"
    );

    // Shift+Up takes the tile above and keeps the two as one column of the grid.
    key(&mut rt, Key::Up, shift());
    assert_eq!(rt.state.selection_len(), 2, "the column between the two");
    let ids = rt.state.selected_ids();
    assert!(ids.contains(&"f:file02.txt".to_owned()));
    assert!(ids.contains(&format!("f:file{below:02}.txt")));
}

#[test]
fn dragging_a_tile_onto_a_folder_tile_moves_it() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-view");
    rt.frame(0.1);

    click(&mut rt, "f:file2.txt");
    let (x0, y0) = rt.find("f:file2.txt").expect("the file tile");
    let (x1, y1) = rt.find("f:alpha").expect("the folder tile");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x1, y1);
    rt.frame(0.05);
    assert!(
        rt.state
            .file_drag
            .as_ref()
            .is_some_and(|drag| drag.dragging),
        "the press became a drag"
    );
    assert_eq!(
        rt.state.drop_row(),
        Some(0),
        "the folder tile is the target"
    );
    rt.pointer_up(x1, y1);
    rt.frame(0.1);

    assert!(dir.path().join("alpha").join("file2.txt").exists());
    assert!(!dir.path().join("file2.txt").exists());
    assert!(rt.state.error.is_none());
}

#[test]
fn the_grid_choice_is_saved_and_restored() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "toggle-view");
    assert!(rt.state.is_grid());
    let config = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(config.contains("grid #true"), "{config}");

    let saved = Settings::at(dir.path().join("config.kdl"));
    assert!(saved.grid);
    assert!(
        App::open_with(dir.path().to_path_buf(), saved).is_grid(),
        "the next launch opens in the grid"
    );
}
