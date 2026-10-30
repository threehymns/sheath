//! The Rules tab's list: a click opens a rule, delete asks first, Escape leaves the form, and
//! the tab says how its rules combine.

use super::helpers::*;
use rune::{Key, KeyEvent, Mods};
use std::fs;

#[test]
fn clicking_a_rule_opens_its_editor() {
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
    click(&mut rt, "rule-second-0");

    assert_eq!(
        rt.state.rule_edit.as_ref().map(|edit| edit.index),
        Some(1),
        "a press anywhere on the row opens that rule's form"
    );
    assert!(
        rt.find("rule-editor-1").is_some(),
        "the form is on screen under the row"
    );
}

#[test]
fn deleting_a_rule_asks_first() {
    let dir = fixture();
    let original = "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n\
                    folder-rule \"second\" {\n    match path=\"/b\"\n    view list\n}\n";
    fs::write(dir.path().join("config.kdl"), original).unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");

    click(&mut rt, "delete-rule-0");
    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "the first press only asks"
    );
    assert!(
        rt.find("confirm-delete-rule-0").is_some(),
        "the row asks before removing the rule"
    );

    click(&mut rt, "cancel-delete-rule-0");
    assert_eq!(
        fs::read_to_string(dir.path().join("config.kdl")).unwrap(),
        original,
        "Cancel leaves the file byte for byte as it was"
    );
    assert!(
        rt.find("confirm-delete-rule-0").is_none(),
        "Cancel closes the question"
    );

    click(&mut rt, "delete-rule-0");
    click(&mut rt, "confirm-delete-rule-0");

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(
        !text.contains("\"first\""),
        "the confirmed rule is gone: {text}"
    );
    assert!(text.contains("\"second\""), "the other rule stayed: {text}");
}

#[test]
fn escape_closes_the_rule_editor_before_the_panel() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    click(&mut rt, "edit-rule-0");
    assert!(rt.state.rule_edit.is_some(), "the form is open");

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);

    assert!(rt.state.rule_edit.is_none(), "Escape closes the form");
    assert_eq!(
        rt.state.settings,
        Some(crate::app::SettingsTab::Rules),
        "and leaves the panel open on the Rules tab behind it"
    );
}

#[test]
fn the_rules_tab_explains_the_order_rules_apply_in() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    view grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    assert!(
        rt.find("rules-help").is_some(),
        "the tab says how rules combine"
    );

    fs::remove_file(dir.path().join("config.kdl")).unwrap();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    assert!(
        rt.find("rules-help").is_none(),
        "the empty state speaks for itself"
    );
}
