//! The rule editor: the form's draft, its Save and Cancel, what it refuses, and what closes it.

use super::helpers::*;
use std::fs;

/// Opens the first rule's editor in the Rules tab.
fn open_rule_editor(rt: &mut rune::Runtime<crate::app::App>) {
    rt.frame(0.0);
    click(rt, "open-settings");
    click(rt, "settings-tab-rules");
    click(rt, "edit-rule-0");
}

#[test]
fn an_editor_closes_when_the_config_file_changes_under_it() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    let wake: rune::Waker = std::sync::Arc::new(|| {});
    rt.state.store.watch(wake);
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"added\" {\n    match path=\"/new\"\n    view list\n}\n\
         folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n",
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
        "folder-rule \"here\" {\n    match path=\"/old\"\n    view grid\n}\n",
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
fn the_rule_editor_cycles_properties_and_the_rule_kind() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    view grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    click(&mut rt, "prop-view");
    assert!(
        matches!(
            rt.state
                .rule_edit
                .as_ref()
                .expect("the form is open")
                .draft
                .props
                .view,
            crate::rules::Prop::Inherit
        ),
        "the cycle moves the draft"
    );
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        !text.contains("view inherit"),
        "the file waits for Save: {text}"
    );

    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("view inherit"), "{text}");

    click(&mut rt, "edit-rule-0");
    click(&mut rt, "rule-editor-kind");
    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("entry-rule"), "the node changed kind: {text}");
    assert!(
        !text.contains("view inherit"),
        "entry rules take no view: {text}"
    );
}

#[test]
fn the_rule_editor_clears_a_property() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    view grid\n    sort size\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

    click(&mut rt, "clear-prop-view");
    assert_eq!(
        rt.state
            .rule_edit
            .as_ref()
            .expect("the form is open")
            .draft
            .props
            .view,
        crate::rules::Prop::Unset,
        "clearing takes the property out of the draft"
    );
    assert!(
        rt.find("clear-prop-view").is_none(),
        "a property the rule no longer sets cannot be cleared again"
    );

    click(&mut rt, "rule-editor-save");
    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        !text.contains("view grid"),
        "the property left the rule: {text}"
    );
    assert!(
        text.contains("sort size"),
        "the other property stays: {text}"
    );
}

#[test]
fn the_rule_editor_adds_and_removes_a_matcher() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/old\"\n    view grid\n}\n",
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

    click(&mut rt, "edit-rule-0");
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
    let original = "folder-rule \"here\" {\n    match path=\"/old\"\n    view grid\n}\n";
    fs::write(dir.path().join("config.kdl"), original).unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    open_rule_editor(&mut rt);

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
        rt.find("rule-editor-dirty").is_some(),
        "the form says it has unsaved changes"
    );
}

#[test]
fn the_rule_editor_refuses_a_match_that_names_nothing() {
    let dir = fixture();
    let original = "folder-rule \"here\" {\n    match path=\"/old\"\n    view grid\n}\n";
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
    let original = "folder-rule \"here\" {\n    match path=\"/old\" min-items=2\n    view grid\n}\n";
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
fn cancelling_the_rule_editor_leaves_the_file_alone() {
    let dir = fixture();
    let original = "folder-rule \"here\" {\n    match path=\"/old\"\n    view grid\n}\n";
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
    click(&mut rt, "rule-editor-cancel");

    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "Cancel writes nothing"
    );
    assert!(rt.state.rule_edit.is_none(), "Cancel closes the form");

    click(&mut rt, "edit-rule-0");
    let reopened = rt.state.rule_edit.as_ref().expect("the form is open");
    assert_eq!(
        reopened.fields.get("m0.path").map(|field| field.text.as_str()),
        Some("/old"),
        "the form opens on the rule as it was"
    );
    assert!(!reopened.dirty(), "there is nothing to save");
}
