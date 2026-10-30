//! Dragging a rule to a new place in the list: the lift, the gap, the drop, and a cancel.

use super::helpers::*;
use std::fs;

#[test]
fn dragging_a_rule_rewrites_the_file_order() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n\
         folder-rule \"second\" {\n    match path=\"/b\"\n    view list\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");

    let (x, y) = rt.find("rule-first-0").expect("the first rule is on screen");
    let (_, below) = rt.find("rule-second-0").expect("the second rule is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.02);
    rt.pointer_move(x, y + 60.0);
    // The rows step aside on a spring, so let the gap finish opening before reading it.
    for _ in 0..10 {
        rt.frame(0.05);
    }

    let (_, lifted) = rt.find("rule-first-0").expect("the dragged row is on screen");
    assert!(
        lifted > y + 40.0,
        "the row in hand follows the pointer: {lifted} from {y}"
    );
    let (_, stepped) = rt.find("rule-second-0").expect("the row below is on screen");
    assert!(
        stepped < below - 20.0,
        "the row it passes steps up into the gap it left: {stepped} from {below}"
    );
    let midway = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    let first = midway.find("first").expect("the first rule is still there");
    let second = midway.find("second").expect("the second rule is still there");
    assert!(
        first < second,
        "nothing is written until the pointer is released: {midway}"
    );
    assert!(rt.state.rule_edit.is_none(), "the drag did not open the editor");

    rt.pointer_up(x, y + 60.0);
    rt.frame(0.02);

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    let first = text.find("first").expect("the first rule is still there");
    let second = text.find("second").expect("the second rule is still there");
    assert!(second < first, "the dragged rule moved above the other: {text}");
    assert!(
        rt.find("rule-first-0").is_some() && rt.find("rule-second-0").is_some(),
        "the rows keep their keys, so the drop lands where the drag showed it"
    );
    assert_eq!(
        rt.state.rule_drag.dragging(),
        None,
        "the gesture is over"
    );
}

#[test]
fn a_press_without_movement_leaves_the_order_alone() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n\
         folder-rule \"second\" {\n    match path=\"/b\"\n    view list\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");

    let (x, y) = rt.find("rule-first-0").expect("the first rule is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.02);
    rt.pointer_up(x, y);
    rt.frame(0.02);

    assert_eq!(
        rt.state.rule_drag.dragging(),
        None,
        "a press is not a drag until the pointer moves"
    );
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    let first = text.find("first").expect("the first rule is still there");
    let second = text.find("second").expect("the second rule is still there");
    assert!(first < second, "a click does not reorder: {text}");
}

#[test]
fn a_cancelled_rule_drag_puts_the_row_back() {
    let mut drag = crate::app::RuleDrag::default();
    drag.follow(0, 2, 60.0, 56.0, 1.0);
    assert_eq!(drag.dragging(), Some(0));
    assert_eq!(drag.shift(0), 60.0, "the row in hand follows the pointer");
    assert_eq!(drag.shift(1), -57.0, "the next row opens the gap");
    assert_eq!(
        drag.end(true),
        None,
        "Escape, or the window losing focus, moves nothing"
    );
    assert_eq!(drag.shift(0), 0.0, "and every row goes back");
    assert_eq!(drag.shift(1), 0.0);
}

#[test]
fn a_drag_does_not_start_while_the_editor_holds_changes() {
    let dir = fixture();
    let original = "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n\
                    folder-rule \"second\" {\n    match path=\"/b\"\n    view list\n}\n";
    fs::write(dir.path().join("config.kdl"), original).unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    click(&mut rt, "edit-rule-0");
    rt.state
        .rule_fields_mut()
        .get_mut("m0.path")
        .expect("the matcher's path field")
        .text = "/typed".to_owned();
    rt.state.apply_rule_fields();
    rt.frame(0.05);

    let (x, y) = rt.find("rule-first-0").expect("the first rule is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.02);
    rt.pointer_move(x, y + 60.0);
    rt.frame(0.05);

    assert!(
        rt.state.rule_edit.is_some(),
        "the form stays open, so its changes are not lost to a drag"
    );
    assert_eq!(rt.state.rule_drag.dragging(), None, "the drag did not start");

    rt.pointer_up(x, y + 60.0);
    rt.frame(0.05);
    assert!(
        rt.state.rule_edit.is_some(),
        "the release does not read as a click on the row and close the form"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "the file is untouched"
    );
}

#[test]
fn a_drag_closes_a_clean_editor_before_reordering() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n\
         folder-rule \"second\" {\n    match path=\"/b\"\n    view list\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    click(&mut rt, "edit-rule-0");
    assert!(rt.state.rule_edit.is_some(), "the form is open");

    let (x, y) = rt.find("rule-first-0").expect("the first rule is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.02);
    rt.pointer_move(x, y + 60.0);
    rt.frame(0.05);
    assert!(
        rt.state.rule_edit.is_none(),
        "the clean form closes so every row is the same height"
    );
    rt.pointer_up(x, y + 60.0);
    rt.frame(0.05);

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    let first = text.find("first").expect("the first rule is still there");
    let second = text.find("second").expect("the second rule is still there");
    assert!(second < first, "the drag reordered the rules: {text}");
}

