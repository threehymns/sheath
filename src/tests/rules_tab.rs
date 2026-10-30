//! The Rules tab: its section per kind, the two Add Rule gestures, the picker that jumps to a
//! rule, and the messages it shows when a file or a command leaves it with nothing to show.

use super::helpers::*;
use std::fs;

#[test]
fn a_broken_config_shows_a_read_only_banner_with_an_open_action() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "settings {\n    show-hidden #true\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");

    assert!(
        rt.find("rules-error").is_some(),
        "the tab says what is wrong"
    );
    assert!(
        rt.find("open-config-editor").is_some(),
        "the banner offers to open the file"
    );
}

#[test]
fn the_picker_lists_every_rule_and_jumps_to_one() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n    layout grid\n}\n\
         folder-rule \"second\" {\n    match path=\"/b\"\n    layout list\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.open_rule_picker();
    rt.frame(0.1);
    assert!(rt.find("rule-picker").is_some(), "the picker is up");

    click(&mut rt, "rule-picker-1");
    assert_eq!(rt.state.settings, Some(crate::app::SettingsTab::Rules));
    assert_eq!(
        rt.state.rule_edit.as_ref().map(|edit| edit.index),
        Some(1),
        "choosing a rule opens it"
    );
    assert!(
        rt.find("rule-editor-1").is_some(),
        "the chosen rule's form is on screen"
    );
    assert!(
        rt.find("rule-picker").is_none(),
        "choosing closes the picker"
    );
}

#[test]
fn the_rules_tab_gives_each_kind_its_own_section_and_add_rule() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/a\"\n    layout grid\n}\n\
         entry-rule \"logs\" {\n    match name=\"*.log\"\n    hidden #true\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");

    assert!(
        rt.find("settings-folder-rules").is_some() && rt.find("settings-entry-rules").is_some(),
        "the page has a section per kind"
    );
    assert!(
        rt.find("rule-here-0").is_some() && rt.find("rule-logs-0").is_some(),
        "each rule is on the page"
    );
    let (_, entry_title) = rt
        .find("settings-entry-rules")
        .expect("the entry section is on screen");
    assert!(
        rt.find("rule-here-0")
            .expect("the folder rule is on screen")
            .1
            < entry_title,
        "the folder rule sits above the entry section"
    );
    assert!(
        rt.find("rule-logs-0")
            .expect("the entry rule is on screen")
            .1
            > entry_title,
        "the entry rule sits under it"
    );
    assert!(
        rt.find("add-folder-rule").is_some() && rt.find("add-entry-rule").is_some(),
        "each section can add a rule of its own kind"
    );
}

#[test]
fn the_entry_sections_add_rule_starts_a_rule_for_the_selected_entry() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.select_id("f:.secret");
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    click(&mut rt, "add-entry-rule");

    assert!(
        rt.state.rule_edit.as_ref().is_some_and(|edit| edit.new),
        "Add Rule opens the form on a rule the file does not hold yet"
    );
    assert!(
        !dir.path().join("config.kdl").exists(),
        "nothing is written until Save"
    );

    click(&mut rt, "rule-editor-save");

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("entry-rule {"), "{text}");
    assert!(text.contains("hidden #false"), "{text}");
    assert!(
        text.contains(".secret"),
        "the selected entry is the match: {text}"
    );
    assert_eq!(rt.state.rules.rules.len(), 1, "{text}");
    assert!(rt.state.rule_edit.is_none(), "saving closes the form");
}

#[test]
fn a_new_rule_lands_after_the_last_rule_of_its_kind() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"first\" {\n    match path=\"/a\"\n}\n\
         entry-rule \"logs\" {\n    match name=\"*.log\"\n    hidden #true\n}\n\
         folder-rule \"second\" {\n    match path=\"/b\"\n}\n\
         entry-rule \"more\" {\n    match name=\"*.tmp\"\n    hidden #true\n}\n",
    )
    .unwrap();

    let (mut app, _path) = app_in_config_dir(&dir);
    app.open_new_rule(crate::rules::Kind::Folder);
    app.save_rule_edit();

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    let doc: kdl::KdlDocument = text.parse().expect("the file still parses");
    let nodes: Vec<(&str, Option<&str>)> = doc
        .nodes()
        .iter()
        .map(|node| {
            (
                node.name().value(),
                node.get(0).and_then(|entry| entry.as_string()),
            )
        })
        .collect();
    assert_eq!(
        nodes,
        vec![
            ("folder-rule", Some("first")),
            ("entry-rule", Some("logs")),
            ("folder-rule", Some("second")),
            ("folder-rule", None),
            ("entry-rule", Some("more")),
        ],
        "the new folder rule lands after the last folder rule, not at the end: {text}"
    );
}

#[test]
fn the_entry_sections_add_rule_is_live_with_nothing_selected() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.selected = None;
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    click(&mut rt, "add-entry-rule");

    let edit = rt.state.rule_edit.as_ref().expect("the form is open");
    assert!(
        edit.new && edit.draft.matches.is_empty(),
        "with nothing selected the draft starts empty, not disabled"
    );

    click(&mut rt, "rule-editor-save");

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("entry-rule"), "{text}");
    assert_eq!(rt.state.rules.rules.len(), 1, "{text}");
}

#[test]
fn each_sections_add_rule_is_inset_like_its_label() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"here\" {\n    match path=\"/a\"\n    layout grid\n}\n\
         entry-rule \"logs\" {\n    match name=\"*.log\"\n    hidden #true\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");

    for (label, card, button) in [
        (
            "settings-folder-rules-label",
            "settings-folder-rules-card",
            "add-folder-rule",
        ),
        (
            "settings-entry-rules-label",
            "settings-entry-rules-card",
            "add-entry-rule",
        ),
    ] {
        let (label, _) = rt.presented(label).expect("the section's label is drawn");
        let (card, _) = rt.presented(card).expect("the section's card is drawn");
        let (button, _) = rt.presented(button).expect("the section's action is drawn");
        let left = label[0] - card[0];
        let right = (card[0] + card[2]) - (button[0] + button[2]);
        assert!(
            (left - right).abs() < 0.5,
            "the action is inset like the label: {right} from the card's edge against {left}"
        );
    }
}

#[test]
fn the_rules_tab_adds_a_rule_for_the_folder_on_screen() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    click(&mut rt, "open-settings");
    click(&mut rt, "settings-tab-rules");
    click(&mut rt, "add-folder-rule");
    assert!(
        rt.state.rule_edit.as_ref().is_some_and(|edit| edit.new),
        "Add Rule opens the form on a rule the file does not hold yet"
    );
    assert!(
        !dir.path().join("config.kdl").exists(),
        "nothing is written until Save"
    );

    click(&mut rt, "rule-editor-save");

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("folder-rule"), "{text}");
    assert!(
        text.contains("layout list") && text.contains("sort name"),
        "the draft starts from the folder on screen and its layout: {text}"
    );
    assert_eq!(rt.state.rules.rules.len(), 1, "{text}");
    let key = format!("rule-{}-0", rt.state.rules.rules[0].summary());

    rt.frame(0.1);
    assert!(
        rt.find(&key).is_some(),
        "the new rule is on screen as {key}"
    );
}

#[test]
fn a_command_with_no_matching_rule_says_so_in_the_tab() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"elsewhere\" {\n    match path=\"/nowhere\"\n    layout grid\n}\n",
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.show_folder_rules();
    rt.frame(0.05);

    assert!(
        rt.find("rules-no-match").is_some(),
        "the tab says nothing matched"
    );
    assert_eq!(rt.state.settings, Some(crate::app::SettingsTab::Rules));
}
