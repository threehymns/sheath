//! The rule form's actions: Save and Cancel in the corner they are cut into, when they are up,
//! how they arrive, and the bite that holds them.

use super::helpers::*;
use rune::{Key, KeyEvent, Mods};
use std::fs;

#[test]
fn the_corner_bite_opens_with_the_buttons_while_its_edge_holds_the_card() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);
    // A change to the draft mounts Save, so the bite takes the room it needs.
    click(&mut rt, "add-match");
    let (form, _) = rt.presented("rule-editor-0").expect("the form is drawn");
    let edge = form[0] + form[2];
    let mut widths = Vec::new();
    for _ in 0..12 {
        let (cut, _) = rt
            .presented("rule-editor-corner")
            .expect("the bite is drawn");
        assert!(
            (cut[0] + cut[2] - edge).abs() < 0.5,
            "the bite ends at the form's right edge: {} from {edge}",
            cut[0] + cut[2]
        );
        widths.push(cut[2]);
        rt.frame(1.0 / 60.0);
    }
    // The bite is drawn into a box that springs, and the drawing stretches to fill it, so the
    // half by the card's edge holds still while the rest opens out to the room Save takes.
    assert!(
        widths[0] + 4.0 < widths[widths.len() - 1],
        "the bite opens from its left edge: {widths:?}"
    );
}

#[test]
fn typing_that_makes_the_draft_dirty_animates_the_save_button_in() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);
    assert!(
        rt.find("rule-editor-save").is_none(),
        "a form that matches the file has nothing to save"
    );

    click(&mut rt, "rule-field-name");
    rt.key(KeyEvent {
        key: Key::Text("x".into()),
        mods: Mods::default(),
    });
    // The keystroke lands in its own frame, at once; Save waits for the one after it.
    rt.frame(0.0);
    assert!(
        rt.state.rule_edit.as_ref().is_some_and(|edit| edit.dirty()),
        "the keystroke reached the field, so the draft differs from the file"
    );
    assert!(
        rt.find("rule-editor-save").is_none(),
        "Save is not drawn in the keystroke's own commit"
    );
    assert!(
        rt.wake_deadline().is_some(),
        "the frame that brings Save in is asked for, not left for the next edit"
    );
    rt.frame(0.0);

    // The keystroke's own commit lands at once, but Save is drawn from what the frame after it
    // noticed, so the bite grows to fit both buttons instead of snapping open at full width.
    let arriving = rt
        .presented("rule-editor-actions")
        .expect("Save is up beside Cancel")
        .0;
    assert!(
        arriving[2] < 118.0,
        "the bite is still opening: {}",
        arriving[2]
    );
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let landed = rt
        .presented("rule-editor-actions")
        .expect("the actions stay")
        .0;
    assert!(
        landed[2] > 121.0,
        "and it opens to fit both buttons: {}",
        landed[2]
    );
}

#[test]
fn an_open_rule_form_takes_the_place_of_its_row() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    assert!(
        rt.find("rule-here-0").is_none(),
        "the row is not drawn under the open form"
    );
    assert!(rt.find("rule-field-name").is_some(), "the form is");

    click(&mut rt, "rule-editor-cancel");
    assert!(
        rt.find("rule-here-0").is_some(),
        "the row comes back when the form closes"
    );
}

#[test]
fn the_rule_forms_actions_sit_on_the_cards_corner() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    rt.state
        .rule_fields_mut()
        .get_mut("name")
        .expect("the name field")
        .text = "edited".to_owned();
    rt.state.apply_rule_fields();
    // A click redraws the form, which is what puts the actions up for the new draft.
    click(&mut rt, "rule-field-name");

    let (card, _) = rt
        .presented("settings-folder-rules-card")
        .expect("the card is drawn");
    let (actions, _) = rt
        .presented("rule-editor-actions")
        .expect("the actions are drawn");
    let (cancel, _) = rt.presented("rule-editor-cancel").expect("Cancel is drawn");
    let (save, _) = rt.presented("rule-editor-save").expect("Save is drawn");
    assert!(
        (cancel[3] - save[3]).abs() <= 0.5,
        "Cancel is the same height as Save: {cancel:?} and {save:?}"
    );
    // The bite is the buttons plus the gap on every side, and the form keeps a 4 pt sliver of
    // card around itself, so the actions end a sliver and a gap inside the card's edges.
    let right = (card[0] + card[2]) - (actions[0] + actions[2]);
    let top = actions[1] - card[1];
    assert!(
        (12.0..=14.0).contains(&right) && (11.0..=13.0).contains(&top),
        "the actions sit in the bite, a gap in from the card: {right}, {top} from {card:?}, {actions:?}"
    );
}
