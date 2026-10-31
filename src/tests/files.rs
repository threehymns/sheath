use super::helpers::*;
use crate::keybinds::Action;
use rune::Tx;
use std::fs;

#[test]
fn new_folder_picks_a_free_name_and_selects_it() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();

    app.new_folder(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New Folder").is_dir());
    assert_eq!(app.selected.as_deref(), Some("f:New Folder"));

    app.new_folder(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New Folder 2").is_dir());
    assert_eq!(app.selected.as_deref(), Some("f:New Folder 2"));
    assert!(names(&app).contains(&"New Folder 2"));
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
    rt.frame(0.1);
    assert!(dir.path().join("New Folder").is_dir());
    assert_eq!(rt.state.selected.as_deref(), Some("f:New Folder"));
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

#[test]
fn the_inspector_swaps_details_without_moving_them() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    assert!(
        rt.presented("inspector")
            .is_some_and(|(_, opacity)| opacity == 1.0),
        "the pane starts on its empty state"
    );

    let (x, y) = rt.find("f:notes.md").expect("the row is on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);

    let mut swapped = 0;
    for e in rt.elements() {
        if e.exiting || !e.path.contains("pane-body/inspector") {
            continue;
        }
        swapped += 1;
        assert_eq!(e.opacity, 1.0, "{} fades in", e.path);
        assert!(!e.moving, "{} glides", e.path);
    }
    assert!(swapped > 3, "the details are drawn: {swapped} elements");
}

#[test]
fn the_inspector_summarises_a_selection_of_several_rows() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());

    assert!(rt.find("selection-summary").is_some());
    assert!(
        rt.find("preview-scroll").is_none(),
        "no single file preview for several rows"
    );
    assert_eq!(rt.state.selection_size(), 3 + 4 + 24);
    assert_eq!(rt.state.selection_folders(), 0);
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
    app.settle_scan();

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
    app.settle_scan();

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
    app.settle_scan();

    assert!(dir.path().join("file2 copy.txt").exists());
    assert!(dir.path().join("file10 copy.txt").exists());
    assert_eq!(app.selection_len(), 2, "both copies are selected");
}

#[test]
fn there_is_no_paste_until_something_is_copied() {
    let dir = fixture();
    let mut app = app(&dir);
    assert!(!app.action_enabled(Action::Paste));
    app.select_id("f:file2.txt");
    app.copy_selection(false);
    assert!(app.action_enabled(Action::Paste));

    app.paste();
    assert!(dir.path().join("file2 copy.txt").exists());
}

#[test]
fn paste_drops_into_the_folder_that_is_selected() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:notes.md");
    app.run_action(Action::Copy, &mut tx);

    app.select_id("f:alpha");
    app.paste();
    app.settle_scan();

    assert!(
        dir.path().join("alpha").join("notes copy.md").exists(),
        "the copy lands inside the chosen folder"
    );
    assert!(!dir.path().join("notes copy.md").exists());
}

#[test]
fn new_file_picks_a_free_name_and_selects_it() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();

    app.new_file(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New File.txt").is_file());
    assert_eq!(app.selected.as_deref(), Some("f:New File.txt"));
    assert_eq!(
        fs::read_to_string(dir.path().join("New File.txt")).unwrap(),
        ""
    );

    app.new_file(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New File 2.txt").is_file());
    assert_eq!(app.selected.as_deref(), Some("f:New File 2.txt"));
}

#[test]
fn folders_first_can_be_turned_off() {
    let dir = fixture();
    fs::create_dir(dir.path().join("zeta")).unwrap();
    let mut app = app(&dir);
    assert_eq!(
        names(&app).last(),
        Some(&"notes.md"),
        "folders sort above files"
    );

    app.toggle_folders_first();
    assert!(!app.config.folders_first);
    assert_eq!(
        names(&app).last(),
        Some(&"zeta"),
        "files come first when the setting is off"
    );
}

#[test]
fn a_folder_size_is_read_on_demand_and_forgotten_with_the_selection() {
    let dir = fixture();
    fs::create_dir(dir.path().join("alpha").join("inner")).unwrap();
    fs::write(dir.path().join("alpha").join("inner").join("a.txt"), "abc").unwrap();
    let mut app = app(&dir);

    app.select_id("f:alpha");
    assert!(
        app.folder_size.is_none(),
        "a folder's size is not read up front"
    );
    app.calculate_folder_size();
    assert_eq!(
        app.folder_size,
        Some((3, 2)),
        "the button counts the file below and the folder around it"
    );

    app.select_id("f:notes.md");
    assert!(
        app.folder_size.is_none(),
        "choosing another row drops the old answer"
    );
}

#[test]
fn a_picture_previews_as_a_picture_and_not_as_its_bytes() {
    let dir = fixture();
    fs::write(dir.path().join("picture.png"), [0x89, b'P', b'N', b'G']).unwrap();
    fs::write(dir.path().join("drawing.svg"), "<svg/>").unwrap();
    let mut app = app(&dir);

    app.select_id("f:picture.png");
    assert!(
        matches!(app.preview, Some(crate::app::Preview::Image)),
        "a PNG is drawn rather than read as text"
    );
    app.select_id("f:drawing.svg");
    assert!(
        matches!(app.preview, Some(crate::app::Preview::Image)),
        "so is a vector drawing"
    );
    app.select_id("f:notes.md");
    assert!(
        matches!(app.preview, Some(crate::app::Preview::Text(_))),
        "text still previews as text"
    );
}
