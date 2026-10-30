//! The rule editor: the form's draft, its Save and Cancel, what it refuses, and what closes it.

use super::helpers::*;
use std::fs;

#[test]
fn an_editor_closes_when_the_config_file_changes_under_it() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    layout grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    // This rule is named "first", so its row is keyed by that name.
    click(&mut rt, "rule-first-0");

    let wake: rune::Waker = std::sync::Arc::new(|| {});
    rt.state.store.watch(wake);
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"added\" {\n    match path=\"/new\"\n    layout list\n}\n\
         folder-rule \"first\" {\n    match path=\"/a\"\n    layout grid\n}\n",
    )
    .unwrap();
    // A frame's `poll` is what notices a file someone else wrote and applies it.
    let mut reloaded = false;
    for _ in 0..100 {
        rt.frame(0.02);
        if rt.state.rule_edit.is_none() {
            reloaded = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        reloaded,
        "the watcher reported the change and the editor closed"
    );
    rt.frame(0.05);
    assert!(
        rt.find("rules-no-match").is_some(),
        "the tab says why the editor closed"
    );

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        text.contains("\"added\""),
        "the rule written in the editor survives: {text}"
    );
    assert!(
        text.contains("\"first\""),
        "and so does the one that was open: {text}"
    );
}

#[test]
fn the_rules_tab_edits_a_rule_in_place() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    layout grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    rt.state
        .rule_fields_mut()
        .get_mut("m0.path")
        .expect("the matcher's path field")
        .text = "/edited/**".to_owned();
    rt.state.apply_rule_fields();
    // A click redraws the form from the state the test just changed by hand, which is what puts
    // Save up for the draft it now holds.
    click(&mut rt, "rule-field-name");

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        !text.contains("/edited/**"),
        "the file waits for Save: {text}"
    );

    click(&mut rt, "rule-editor-save");

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("path=\"/edited/**\""), "{text}");
    assert_eq!(
        rt.state.rules.rules[0].matches[0].path.as_deref(),
        Some("/edited/**"),
        "the open folder reads the edit back: {text}"
    );
    assert!(rt.state.rule_edit.is_none(), "saving closes the form");
}

#[test]
fn the_rule_editor_picks_a_property_value_from_a_list() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    layout list\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    // The field beside `layout` lists the values the file accepts: list, grid, inherit.
    click(&mut rt, "prop-select-layout-trigger");
    assert_eq!(
        rt.state.rule_edit.as_ref().expect("the form is open").menu,
        Some(crate::app::RuleMenu::Prop(crate::app::RuleProp::Layout)),
        "the field opens the property's list"
    );
    click(&mut rt, "option1");
    assert_eq!(
        rt.state
            .rule_edit
            .as_ref()
            .expect("the form is open")
            .draft
            .props
            .layout,
        crate::rules::Prop::Set(crate::app::Layout::Grid),
        "picking a value moves the draft"
    );
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        !text.contains("layout grid"),
        "the file waits for Save: {text}"
    );

    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("layout grid"), "{text}");
}

#[test]
fn the_rule_editor_removes_a_property_from_the_rule() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    layout grid\n    sort size\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    // The line reveals its remove button while the pointer rests on it.
    let (x, y) = rt.find("prop-layout").expect("the property has a line");
    rt.pointer_move(x, y);
    rt.frame(0.1);
    click(&mut rt, "clear-prop-layout");
    assert_eq!(
        rt.state
            .rule_edit
            .as_ref()
            .expect("the form is open")
            .draft
            .props
            .layout,
        crate::rules::Prop::Unset,
        "removing takes the property out of the draft"
    );
    assert!(
        rt.find("prop-layout").is_none(),
        "and its line off the form"
    );
    assert!(
        rt.find("clear-prop-layout").is_none(),
        "a property the rule no longer sets has nothing to remove"
    );

    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        !text.contains("layout grid"),
        "the property left the rule: {text}"
    );
    assert!(
        text.contains("sort size"),
        "the other property stays: {text}"
    );
}

#[test]
fn the_rule_editor_adds_a_property_from_the_menu() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);
    // The card grows to hold the form on a spring, so the section under it lands a frame or two
    // later; the press waits for the page to settle.
    rt.frame(0.3);

    assert!(
        rt.find("prop-sort").is_none(),
        "a property the rule leaves unset has no line"
    );
    // Add Property lists what the rule does not set yet: layout, sort, direction, show-hidden.
    click(&mut rt, "add-property-trigger");
    assert_eq!(
        rt.state.rule_edit.as_ref().expect("the form is open").menu,
        Some(crate::app::RuleMenu::AddProperty),
        "the button opens the menu of properties"
    );
    click(&mut rt, "option1");
    assert_eq!(
        rt.state
            .rule_edit
            .as_ref()
            .expect("the form is open")
            .draft
            .props
            .sort,
        crate::rules::Prop::Set(crate::rules::SortKey::Name),
        "adding sets the property"
    );
    assert!(
        rt.find("prop-sort").is_some(),
        "and puts a line for it on the form"
    );

    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("sort name"), "the rule took it: {text}");
}

#[test]
fn the_rule_editor_adds_and_removes_a_matcher() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    layout grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    click(&mut rt, "add-match");
    assert_eq!(
        rt.state
            .rule_edit
            .as_ref()
            .expect("the form is open")
            .draft
            .matches
            .len(),
        2
    );
    rt.state
        .rule_fields_mut()
        .get_mut("m1.path")
        .expect("the new matcher's path field")
        .text = "~/second".to_owned();
    rt.state.apply_rule_fields();

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        !text.contains("~/second"),
        "the file waits for Save: {text}"
    );

    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("path=\"~/second\""), "{text}");

    click(&mut rt, "rule-here-0");
    // The matcher's line reveals its remove button while the pointer rests on it.
    let (x, y) = rt
        .find("rule-field-m1.path")
        .expect("the matcher has a line");
    rt.pointer_move(x, y);
    rt.frame(0.1);
    click(&mut rt, "remove-m1");
    assert_eq!(
        rt.state
            .rule_edit
            .as_ref()
            .expect("the form is open")
            .draft
            .matches
            .len(),
        1
    );

    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(!text.contains("~/second"), "the removal is saved: {text}");
}

#[test]
fn a_rule_editor_writes_nothing_until_it_is_saved() {
    let dir = fixture();
    let original = "folder-rule \"here\" {\n    match path=\"/old\"\n    layout grid\n}\n";
    fs::write(dir.path().join("config.kdl"), original).unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);
    assert!(
        rt.find("rule-editor-save").is_none(),
        "a form with no changes has nothing to save"
    );
    assert!(
        rt.find("rule-editor-cancel").is_some(),
        "and Cancel is there to close it"
    );

    rt.state
        .rule_fields_mut()
        .get_mut("m0.path")
        .expect("the matcher's path field")
        .text = "/typed".to_owned();
    rt.state.apply_rule_fields();
    // A click redraws the tab from the state the test just changed by hand.
    click(&mut rt, "rule-field-name");

    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "typing does not touch the file"
    );
    assert_eq!(
        rt.state.rules.rules[0].matches[0].path.as_deref(),
        Some("/old"),
        "the folder on screen still resolves the saved rule"
    );
    assert!(
        rt.state
            .rule_edit
            .as_ref()
            .expect("the form is open")
            .dirty(),
        "the draft differs from the rule as it opened"
    );
    assert!(
        rt.find("rule-editor-save").is_some(),
        "the form offers Save once it has changes"
    );
}

#[test]
fn the_rule_editor_refuses_a_match_that_names_nothing() {
    let dir = fixture();
    let original = "folder-rule \"here\" {\n    match path=\"/old\"\n    layout grid\n}\n";
    fs::write(dir.path().join("config.kdl"), original).unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    click(&mut rt, "add-match");
    click(&mut rt, "rule-editor-save");

    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "a match that names nothing is not written"
    );
    assert!(rt.state.rule_edit.is_some(), "the form stays open");
    assert!(
        rt.find("rule-editor-error").is_some(),
        "the form says why it refused"
    );
}

#[test]
fn the_rule_editor_refuses_a_count_that_is_not_a_number() {
    let dir = fixture();
    let original =
        "folder-rule \"here\" {\n    match path=\"/old\" min-items=2\n    layout grid\n}\n";
    fs::write(dir.path().join("config.kdl"), original).unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    rt.state
        .rule_fields_mut()
        .get_mut("m0.min-items")
        .expect("the matcher's min-items field")
        .text = "abc".to_owned();
    rt.state.apply_rule_fields();
    click(&mut rt, "rule-field-name");
    assert!(
        rt.find("rule-editor-error").is_none(),
        "a draft is not judged while it is typed"
    );
    click(&mut rt, "rule-editor-save");

    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "the file still holds the number it had"
    );
    assert!(rt.state.rule_edit.is_some(), "the form stays open");
    assert!(
        rt.find("rule-editor-error").is_some(),
        "the form says the count needs a whole number"
    );
}

#[test]
fn a_refused_save_says_why_without_moving_the_form() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    layout grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    // A match that names nothing is the longest sentence the form refuses with.
    click(&mut rt, "add-match");
    assert!(
        rt.find("rule-editor-error").is_none(),
        "adding a match says nothing until Save is pressed"
    );
    let (before, _) = rt
        .presented("rule-field-m0.path")
        .expect("the first matcher's line is drawn");
    let (slot, _) = rt
        .presented("rule-editor-problem")
        .expect("the line for a refusal keeps its place");

    click(&mut rt, "rule-editor-save");

    let (message, _) = rt
        .presented("rule-editor-error")
        .expect("the form says why it refused");
    assert!(
        message[3] <= slot[3] + 0.5,
        "the message is one line in the line kept for it: {message:?} in {slot:?}"
    );
    assert!(
        message[0] + message[2] <= slot[0] + slot[2] + 0.5,
        "and fits across it, so nothing of it is cut off: {message:?} in {slot:?}"
    );
    let (after, _) = rt
        .presented("rule-field-m0.path")
        .expect("the first matcher's line is still drawn");
    assert!(
        (before[1] - after[1]).abs() < 0.5,
        "the rows under the message stayed where they were: {before:?} then {after:?}"
    );
}

#[test]
fn cancelling_the_rule_editor_leaves_the_file_alone() {
    let dir = fixture();
    let original = "folder-rule \"here\" {\n    match path=\"/old\"\n    layout grid\n}\n";
    fs::write(dir.path().join("config.kdl"), original).unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    rt.state
        .rule_fields_mut()
        .get_mut("m0.path")
        .expect("the matcher's path field")
        .text = "/changed".to_owned();
    rt.state.apply_rule_fields();
    click(&mut rt, "rule-field-name");
    click(&mut rt, "rule-editor-cancel");

    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "Cancel writes nothing"
    );
    assert!(rt.state.rule_edit.is_none(), "Cancel closes the form");

    click(&mut rt, "rule-here-0");
    let reopened = rt.state.rule_edit.as_ref().expect("the form is open");
    assert_eq!(
        reopened
            .fields
            .get("m0.path")
            .map(|field| field.text.as_str()),
        Some("/old"),
        "the form opens on the rule as it was"
    );
    assert!(!reopened.dirty(), "there is nothing to save");
}

#[test]
fn the_rule_editor_says_which_sections_are_empty() {
    let dir = fixture();
    fs::write(dir.path().join("config.kdl"), "folder-rule \"here\" {\n}\n").unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    assert!(
        rt.find("no-matches").is_some(),
        "the match section says it is empty"
    );
    assert!(
        rt.find("no-excludes").is_some(),
        "and so does the exclude section"
    );
    assert!(
        rt.find("no-properties").is_some(),
        "and the properties section"
    );

    click(&mut rt, "add-match");
    assert!(
        rt.find("no-matches").is_none(),
        "a match fills the match section"
    );
    click(&mut rt, "add-exclude");
    assert!(
        rt.find("no-excludes").is_none(),
        "an exclude fills the exclude section"
    );
}
