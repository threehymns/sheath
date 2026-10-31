//! The grid layout: switching to it, clicking and walking its tiles, and remembering the choice.

use super::helpers::*;
use crate::app::{App, GRID_TOP_MARGIN, Layout, TILE_GAP, TILE_HEIGHT, TILE_WIDTH};
use rune::{Key, KeyEvent, Mods};
use rune_kit::SortDir;
use std::fs;

#[test]
fn a_layout_that_came_with_the_folder_lands_without_a_fade() {
    let dir = fixture();
    let alpha = dir.path().join("alpha");
    let mut app = app(&dir);
    // The folder was left in the grid, so opening it brings the grid with it.
    app.views.set(
        &alpha,
        crate::rules::Resolved {
            layout: Some(Layout::Grid),
            ..Default::default()
        },
    );

    app.navigate(alpha);
    assert_eq!(app.layout, Layout::Grid);
    assert!(app.layout_cut, "the folder's own layout lands at once");

    // Asking for the other layout is the user's change, and keeps its fade.
    app.toggle_layout();
    assert_eq!(app.layout, Layout::List);
    assert!(!app.layout_cut);
}

#[test]
fn the_toolbar_switches_between_the_list_and_the_grid() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    assert_eq!(rt.state.layout, Layout::List);
    assert!(
        rt.find("header").is_some(),
        "the list has its column header"
    );

    click(&mut rt, "toggle-layout");
    assert_eq!(rt.state.layout, Layout::Grid);
    assert!(rt.find("file-grid").is_some(), "the grid is on screen");
    assert!(rt.find("header").is_none(), "the grid has no column header");
    assert!(
        rt.find("f:notes.md").is_some(),
        "the file has a tile on screen"
    );

    click(&mut rt, "toggle-layout");
    assert_eq!(rt.state.layout, Layout::List);
    assert!(rt.find("file-grid").is_none());
    assert!(rt.find("header").is_some());
}

#[test]
fn clicking_a_tile_selects_it() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-layout");
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
    click(&mut rt, "toggle-layout");
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
    click(&mut rt, "toggle-layout");
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
fn a_click_in_the_gaps_between_tiles_clears_the_selection() {
    let dir = long_folder(24);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-layout");
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }

    let (x, y) = rt.find("f:file00.txt").expect("the first tile");
    // The gap after the tile in its grid row, and the one below it before the next row.
    for (gx, gy) in [
        (x + TILE_WIDTH / 2.0 + TILE_GAP / 2.0, y),
        (x, y + TILE_HEIGHT / 2.0 + TILE_GAP / 2.0),
    ] {
        rt.state.select_id("f:file05.txt");
        rt.frame(0.05);
        assert_eq!(
            rt.state.selection_len(),
            1,
            "the click starts from a selection"
        );

        rt.pointer_down(gx, gy);
        rt.frame(0.05);
        rt.pointer_up(gx, gy);
        rt.frame(0.1);
        assert_eq!(rt.state.selected, None, "nothing is picked at {gx},{gy}");
        assert_eq!(
            rt.state.selection_len(),
            0,
            "a click between tiles clears the selection"
        );
    }
}

#[test]
fn the_gaps_between_tiles_start_a_box_rather_than_a_drag() {
    let dir = long_folder(24);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-layout");
    rt.frame(0.1);

    let (x, y) = rt.find("f:file00.txt").expect("the first tile");
    // The gap after the tile in its grid row, and the one below it before the next row.
    for (gx, gy) in [
        (x + TILE_WIDTH / 2.0 + TILE_GAP / 2.0, y),
        (x, y + TILE_HEIGHT / 2.0 + TILE_GAP / 2.0),
    ] {
        rt.pointer_down(gx, gy);
        rt.frame(0.05);
        rt.pointer_move(gx + 30.0, gy + 12.0);
        rt.frame(0.05);
        assert!(
            rt.state.marquee.is_some(),
            "a press at {gx},{gy} starts a selection box"
        );
        assert!(
            rt.state.file_drag.is_none(),
            "no tile is picked up at {gx},{gy}"
        );
        assert!(rt.find("drag-badge").is_none(), "nothing is dragged");
        rt.pointer_up(gx + 30.0, gy + 12.0);
        rt.frame(0.1);
    }
}

#[test]
fn a_box_sweeps_only_the_tiles_it_covers() {
    let dir = long_folder(24);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-layout");
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }

    let (x, y) = rt.find("f:file00.txt").expect("the first tile");
    let edge = x + TILE_WIDTH / 2.0;
    // A box that stays in the gap after the first tile covers neither tile.
    rt.pointer_down(edge + 2.0, y);
    rt.frame(0.05);
    rt.pointer_move(edge + 5.0, y + 6.0);
    rt.frame(0.05);
    assert_eq!(
        rt.state.selection_len(),
        0,
        "a box drawn only in the gap takes nothing"
    );
    rt.pointer_up(edge + 5.0, y + 6.0);
    rt.frame(0.1);

    // One that reaches into the next tile takes it, and not the tile before the gap.
    rt.pointer_down(edge + 2.0, y);
    rt.frame(0.05);
    rt.pointer_move(edge + TILE_GAP + 6.0, y + 6.0);
    rt.frame(0.05);
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file01.txt"],
        "only the tile the box covers"
    );
    rt.pointer_up(edge + TILE_GAP + 6.0, y + 6.0);
    rt.frame(0.1);
}

#[test]
fn the_grid_toolbar_sorts_from_the_tiles() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.frame(0.1);
    let list_top = rt.state.rows_view[1];

    click(&mut rt, "toggle-layout");
    rt.frame(0.1);
    assert!(
        rt.find("grid-sort-name").is_some(),
        "the grid offers the sort controls its header used to"
    );
    assert_eq!(
        rt.state.rows_view[1], list_top,
        "the viewport starts under the sort toolbar, as the list's does"
    );
    let (toolbar, _) = rt
        .presented("grid-toolbar")
        .expect("the sort toolbar is drawn");
    let (tile, _) = rt.presented("f:alpha").expect("the first tile is drawn");
    assert!(
        (tile[1] - (toolbar[1] + toolbar[3]) - GRID_TOP_MARGIN).abs() < 0.5,
        "the tiles sit a margin under the sort toolbar: {} below it",
        tile[1] - (toolbar[1] + toolbar[3])
    );

    click(&mut rt, "grid-sort-size");
    assert_eq!(
        names(&rt.state),
        ["alpha", "file2.txt", "file10.txt", "notes.md"]
    );
    assert_eq!(rt.state.sort_column(), ("size", SortDir::Ascending));

    click(&mut rt, "grid-sort-size");
    assert_eq!(
        names(&rt.state),
        ["alpha", "notes.md", "file10.txt", "file2.txt"],
        "a second click reverses the sort"
    );
    assert_eq!(rt.state.sort_column(), ("size", SortDir::Descending));

    click(&mut rt, "toggle-layout");
    rt.frame(0.1);
    assert!(
        rt.find("size").is_some(),
        "the table's header shows the same sort"
    );
    assert_eq!(rt.state.sort_column(), ("size", SortDir::Descending));
}

#[test]
fn the_sort_chevron_turns_over_when_the_direction_reverses() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-layout");
    rt.frame(0.1);

    let turn = |rt: &rune::Runtime<App>| {
        rt.presented_pose("grid-sort-size/sort")
            .expect("the sorted column shows a chevron")
            .rotation()
    };
    let settle = |rt: &mut rune::Runtime<App>| {
        for _ in 0..40 {
            rt.frame(1.0 / 60.0);
        }
    };

    click(&mut rt, "grid-sort-size");
    settle(&mut rt);
    assert!(
        (turn(&rt) - std::f32::consts::PI).abs() < 0.01,
        "an ascending sort points up: {}",
        turn(&rt)
    );

    // Press and release without the helper's settle, to catch the arrow partway round.
    let (x, y) = rt.find("grid-sort-size").expect("the sort control");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
    let turning = turn(&rt);
    assert!(
        turning > 0.01 && turning < std::f32::consts::PI - 0.01,
        "the chevron turns rather than swapping glyphs: {turning}"
    );

    settle(&mut rt);
    assert!(
        turn(&rt).abs() < 0.01,
        "a descending sort points down: {}",
        turn(&rt)
    );
}

#[test]
fn the_band_under_the_sort_bar_is_empty_space_and_the_card_starts_at_its_edge() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "toggle-layout");
    rt.frame(0.1);

    // The margin under the bar is inside the viewport: a press there is empty space, not the
    // tile row the raw point would round to.
    let (toolbar, _) = rt.presented("grid-toolbar").expect("the sort toolbar");
    let band = toolbar[1] + toolbar[3] + GRID_TOP_MARGIN * 0.5;
    let x = rt.state.rows_view[0] + 40.0;
    rt.state.select_id("f:notes.md");
    rt.frame(0.05);
    rt.pointer_down(x, band);
    rt.frame(0.05);
    rt.pointer_move(x + 40.0, band);
    rt.frame(0.05);
    assert!(
        rt.state.marquee.is_some(),
        "the margin under the sort bar starts a box"
    );
    assert!(rt.state.file_drag.is_none(), "it picks no tile up");
    assert_eq!(rt.state.selection_len(), 0, "and clears the selection");
    rt.pointer_up(x + 40.0, band);
    rt.frame(0.1);

    // The card is the tile's handle right to its edge, padding and all.
    let (tile, _) = rt.presented("f:alpha").expect("the first tile");
    let (tx, ty, _, th) = (tile[0], tile[1], tile[2], tile[3]);
    rt.pointer_down(tx + 2.0, ty + th * 0.5);
    rt.frame(0.05);
    rt.pointer_move(tx + 80.0, ty + th * 0.5 + 20.0);
    rt.frame(0.05);
    assert!(
        rt.state
            .file_drag
            .as_ref()
            .is_some_and(|drag| drag.dragging),
        "the tile's own edge picks it up"
    );
    rt.pointer_up(tx + 80.0, ty + th * 0.5 + 20.0);
    rt.frame(0.1);
}

#[test]
fn the_grid_choice_is_remembered_for_the_folder() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "toggle-layout");
    assert!(rt.state.is_grid());
    let views = fs::read_to_string(dir.path().join("views.kdl")).unwrap();
    assert!(views.contains("layout grid"), "{views}");

    let remembered = crate::views::Views::at(dir.path().join("views.kdl")).get(dir.path());
    assert_eq!(remembered.layout, Some(crate::app::Layout::Grid));
}
