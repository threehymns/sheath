use super::helpers::*;
use rune::{Key, KeyEvent, Mods};

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
fn nothing_in_the_file_area_takes_focus_but_its_viewport() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    for layout in ["list", "grid"] {
        if layout == "grid" {
            click(&mut rt, "toggle-layout");
            rt.frame(0.2);
        }
        let mut focusable: Vec<String> = rt
            .elements()
            .into_iter()
            .filter(|e| {
                e.focusable && (e.path.contains("/file-table") || e.path.contains("/file-grid"))
            })
            .map(|e| e.key.unwrap_or_else(|| e.path.clone()))
            .collect();
        focusable.sort();
        assert_eq!(
            focusable,
            ["files-table"],
            "{layout}: the file area's focus targets"
        );
    }
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
fn ctrl_shift_clicking_keeps_the_selection_and_adds_a_range() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:file10.txt", shift());
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
