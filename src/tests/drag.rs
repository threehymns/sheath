use super::helpers::*;
use rune::{Mods, PhysicalInput};
use std::fs;

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

    let (x0, y0) = name_point(&rt, "f:file2.txt");
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
fn a_drag_from_an_unselected_rows_columns_sweeps_a_box() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.frame(0.1);

    let (x, y) = rt.find("f:file2.txt").expect("the row is on screen");
    let view = rt.state.rows_view;
    let size_x = view[0] + view[2] - rt.state.date_width - rt.state.size_width / 2.0;
    let date_x = view[0] + view[2] - rt.state.date_width / 2.0;
    for (column, at) in [("size", size_x), ("date", date_x)] {
        assert!(
            at > x,
            "the {column} column is right of the name cell: {at} against {x}, view {view:?}"
        );

        rt.pointer_down(at, y);
        rt.pointer_up(at, y);
        rt.frame(0.1);
        assert_eq!(
            rt.state.selected.as_deref(),
            Some("f:file2.txt"),
            "a click in the {column} column selects the row"
        );

        rt.state.select_id("f:notes.md");
        rt.frame(0.05);
        rt.pointer_down(at, y);
        rt.frame(0.05);
        rt.pointer_move(at, y + crate::app::ROW_HEIGHT);
        rt.frame(0.05);
        assert!(
            rt.state.marquee.is_some(),
            "a drag in the {column} column draws a box"
        );
        assert!(
            rt.state.file_drag.is_none(),
            "no row is picked up in the {column} column"
        );
        assert!(rt.find("drag-badge").is_none(), "nothing is dragged");
        rt.pointer_up(at, y + crate::app::ROW_HEIGHT);
        rt.frame(0.1);
        assert_eq!(
            rt.state.selection_len(),
            2,
            "the box swept the pressed row and the one below it"
        );
        assert!(
            dir.path().join("file2.txt").exists(),
            "the file stayed where it was"
        );
    }
}

#[test]
fn a_drag_on_a_selected_row_picks_it_up_from_anywhere() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.frame(0.1);

    let (row, _) = rt.presented("f:file2.txt").expect("the row is on screen");
    let y = row[1] + row[3] * 0.5;
    let view = rt.state.rows_view;
    let size_x = view[0] + view[2] - rt.state.date_width - rt.state.size_width / 2.0;
    let (name, _) = rt.presented("name-f:file2.txt").expect("the name is drawn");
    let blank = name[0] + name[2] + 20.0;
    let cell_right = view[0] + view[2] - rt.state.size_width - rt.state.date_width;
    assert!(
        blank < cell_right,
        "the press lands past the name, inside the name column: {blank} against {cell_right}"
    );

    for at in [blank, size_x] {
        rt.state.select_id("f:file2.txt");
        rt.frame(0.1);
        rt.pointer_down(at, y);
        rt.frame(0.05);
        rt.pointer_move(at + 40.0, y + 12.0);
        rt.frame(0.05);
        assert!(
            rt.state
                .file_drag
                .as_ref()
                .is_some_and(|drag| drag.dragging),
            "a selected row is picked up from {at}"
        );
        assert!(
            rt.state.marquee.is_none(),
            "no box starts on a selected row"
        );
        assert!(
            rt.find("drag-badge").is_some(),
            "the drag says what it carries"
        );

        rt.pointer_move(at, y);
        rt.frame(0.05);
        rt.pointer_up(at, y);
        rt.frame(0.1);
        assert!(dir.path().join("file2.txt").is_file(), "nothing moved");
    }
}

#[test]
fn dragging_a_row_against_an_edge_scrolls_the_rows_under_it() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..40 {
        fs::create_dir(dir.path().join(format!("d{i:02}"))).unwrap();
    }
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    let (x, y) = name_point(&rt, "f:d00");
    let [_, top, _, height] = rt.presented("files-table").expect("the list").0;
    let edge = top + height - 4.0;

    rt.pointer_down(x, y);
    rt.frame(0.05);
    rt.pointer_move(x, edge);
    rt.frame(0.05);
    let aimed = rt
        .state
        .drop_row()
        .expect("a folder row is under the pointer");
    let before = scroll_offset(&rt);
    for _ in 0..8 {
        rt.frame(1.0 / 60.0);
    }
    let after = scroll_offset(&rt);
    let passed = rt
        .state
        .drop_row()
        .expect("a folder row is under the pointer");
    assert!(
        after > before + 10.0,
        "the rows scrolled under the drag: {before} -> {after}"
    );
    assert!(
        passed > aimed,
        "the drop follows the rows that passed under the pointer: {aimed} -> {passed}"
    );

    rt.pointer_move(x, top + 4.0);
    for _ in 0..4 {
        rt.frame(1.0 / 60.0);
    }
    assert!(
        scroll_offset(&rt) < after - 10.0,
        "the top edge scrolled back up: {after} -> {}",
        scroll_offset(&rt)
    );

    rt.pointer_move(x, top + 200.0);
    rt.frame(1.0 / 60.0);
    let [x1, y1, _, h] = rt.presented("f:d05").expect("a folder row is on screen").0;
    let (x1, y1) = (x1 + 40.0, y1 + h * 0.5);
    rt.pointer_move(x1, y1);
    rt.frame(1.0 / 60.0);
    rt.pointer_up(x1, y1);
    rt.frame(0.1);
    assert!(
        dir.path().join("d05").join("d00").is_dir(),
        "the row was dropped on the folder it ended over"
    );
}

#[test]
fn an_option_drag_draws_a_box_over_the_rows_and_scrolls_at_the_edge() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    let [x, top, _, height] = rt.presented("files-table").expect("the list").0;
    let x = x + 40.0;

    rt.physical_input(PhysicalInput::Modifiers(Mods {
        alt: true,
        ..Mods::default()
    }));
    rt.pointer_down(x, top + 10.0);
    rt.frame(0.05);
    rt.pointer_move(x, top + 100.0);
    rt.frame(0.05);
    assert!(
        rt.state.marquee.is_some(),
        "Option starts a box over the rows"
    );
    assert!(rt.state.file_drag.is_none(), "no row is being dragged");
    let swept = rt.state.selection_len();
    assert!(swept > 1, "the box swept the rows it covers: {swept}");

    rt.pointer_move(x, top + height - 4.0);
    rt.frame(0.05);
    let before = scroll_offset(&rt);
    for _ in 0..6 {
        rt.frame(1.0 / 60.0);
    }
    assert!(
        scroll_offset(&rt) > before + 10.0,
        "the box's edge scrolled the rows: {before} -> {}",
        scroll_offset(&rt)
    );
    assert!(
        rt.state.selection_len() > swept,
        "the box took the rows that scrolled under it: {swept} -> {}",
        rt.state.selection_len()
    );

    rt.pointer_up(x, top + height - 4.0);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(0.1);
    assert!(rt.state.marquee.is_none(), "the box ends with the button");
    assert!(
        rt.state.selection_len() > swept,
        "the release keeps what the box swept"
    );
}

#[test]
fn a_row_drag_shows_a_badge_and_no_selection_box() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x0, y0) = name_point(&rt, "f:file2.txt");
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
    let rect = [20.0, 620.0, 180.0, 40.0];
    rt.state.places_at.insert(
        "test-place".to_owned(),
        crate::app::PlaceAt {
            rect,
            path: destination.clone(),
            section: crate::places::Section::Favourites,
            index: 0,
        },
    );

    click(&mut rt, "f:file2.txt");
    let (x0, y0) = name_point(&rt, "f:file2.txt");
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
fn the_name_is_the_handle_and_the_space_past_it_is_canvas() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.frame(0.1);

    let (name, _) = rt.presented("name-f:file2.txt").expect("the name is drawn");
    let right = name[0] + name[2];
    let y = name[1] + name[3] * 0.5;
    let cell_right =
        rt.state.rows_view[0] + rt.state.rows_view[2] - rt.state.size_width - rt.state.date_width;
    assert!(
        right + 40.0 < cell_right,
        "the name leaves blank column after it: {right} + 40 against {cell_right}"
    );

    rt.state.select_id("f:notes.md");
    rt.frame(0.05);
    let blank = right + 20.0;
    rt.pointer_down(blank, y);
    rt.frame(0.05);
    rt.pointer_move(blank, y + crate::app::ROW_HEIGHT);
    rt.frame(0.05);
    assert!(
        rt.state.file_drag.is_none(),
        "the blank space picks nothing up"
    );
    assert!(rt.state.marquee.is_some(), "it draws a box instead");
    assert!(rt.find("drag-badge").is_none(), "nothing is dragged");
    rt.pointer_up(blank, y + crate::app::ROW_HEIGHT);
    rt.frame(0.1);
    assert!(
        dir.path().join("file2.txt").is_file(),
        "the file stayed where it was"
    );

    rt.pointer_down(right + 3.0, y);
    rt.frame(0.05);
    rt.pointer_move(right + 63.0, y);
    rt.frame(0.05);
    assert!(
        rt.state
            .file_drag
            .as_ref()
            .is_some_and(|drag| drag.dragging),
        "3 points past the name still picks the row up"
    );
    rt.pointer_up(right + 63.0, y);
    rt.frame(0.1);
    assert!(dir.path().join("file2.txt").is_file(), "and moved nothing");
}

#[test]
fn a_drag_that_comes_back_to_its_start_is_a_click() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x, y) = name_point(&rt, "f:file2.txt");
    rt.pointer_down(x, y);
    rt.frame(0.05);
    rt.pointer_move(x + 60.0, y + 60.0);
    rt.frame(0.05);
    assert!(
        rt.state
            .file_drag
            .as_ref()
            .is_some_and(|drag| drag.dragging),
        "the press became a drag"
    );

    rt.pointer_move(x + 1.0, y + 1.0);
    rt.frame(0.05);
    assert!(
        rt.state
            .file_drag
            .as_ref()
            .is_some_and(|drag| !drag.dragging),
        "coming back to the press releases the drag it armed"
    );
    assert!(rt.find("drag-badge").is_none(), "nothing is carried");
    rt.pointer_up(x + 1.0, y + 1.0);
    rt.frame(0.1);

    assert!(dir.path().join("file2.txt").is_file(), "no file moved");
    assert_eq!(rt.state.selected.as_deref(), Some("f:file2.txt"));
    assert_eq!(rt.state.selection_len(), 1);
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
