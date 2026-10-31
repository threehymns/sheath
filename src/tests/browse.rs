//! Listing, sorting, filtering and navigating the folder on screen.

use super::helpers::*;
use crate::app::{Preview, SYNC_SCAN_LIMIT};
use rune::{Key, KeyEvent, Mods};
use rune_kit::SortDir;
use std::fs;

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

#[test]
fn a_reload_reads_a_big_folder_off_the_main_thread() {
    // Past the fast path's limit, so the read has to leave the main thread.
    let dir = long_folder(SYNC_SCAN_LIMIT + 1);
    let mut app = app(&dir);
    fs::write(dir.path().join("late.txt"), "late").unwrap();

    app.reload();
    assert!(
        !names(&app).contains(&"late.txt"),
        "reload returns before the read does"
    );
    assert!(app.is_scanning(), "the read is still in flight");

    app.settle_scan();
    assert!(names(&app).contains(&"late.txt"), "the read lands its rows");
}

#[test]
fn a_small_folder_switch_has_its_rows_at_once() {
    let dir = fixture();
    let mut app = app(&dir);
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("one.txt"), "1").unwrap();

    app.navigate(sub);
    // Read in the handler: the very next frame already draws the new folder's rows, so
    // there is no frame showing the list empty.
    assert!(!app.is_scanning());
    assert_eq!(names(&app), ["one.txt"]);
    assert_eq!(app.phrase_for_count(), "1 item");
}

#[test]
fn a_sort_changed_while_the_read_runs_applies_when_it_lands() {
    // Past the fast path, so the read really is in flight while the header is clicked.
    let dir = long_folder(SYNC_SCAN_LIMIT + 1);
    let mut app = app(&dir);
    // The read is asked for with the folder's default name/ascending sort...
    app.reload();
    // ...and the header is clicked while it runs, as a user would: the rows must arrive in
    // the sort that is showing, not the one the read applied.
    app.sort_by("name");
    app.settle_scan();
    let names = names(&app);
    assert_eq!(names.first().copied(), Some("file2000.txt"));
    assert_eq!(names.last().copied(), Some("file00.txt"));
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
