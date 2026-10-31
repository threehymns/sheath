//! A config file the app cannot read: what the toast says about it, and what its Copy hands over.

use super::helpers::*;
use crate::config;
use std::fs;

#[test]
fn a_file_that_does_not_parse_names_its_place() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "settings {\n    grid #true\n}\n}\n",
    )
    .unwrap();
    let store = config::Store::at(dir.path().to_path_buf());
    let problem = store
        .problem(config::CONFIG)
        .expect("the file does not parse");
    assert!(
        problem.detail.starts_with("line "),
        "the one line names the place: {}",
        problem.detail
    );
    assert!(
        problem
            .report
            .contains(&dir.path().join("config.kdl").display().to_string()),
        "the report names the file: {}",
        problem.report
    );
    assert!(
        problem.report.contains("what this file holds"),
        "the report says what the file is meant to be: {}",
        problem.report
    );
}

#[test]
fn the_broken_file_raises_a_toast_in_the_corner() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "settings {\n    grid #true\n}\n}\n",
    )
    .unwrap();
    let (app, _path) = app_in_config_dir(&dir);
    assert_eq!(app.notices.len(), 1, "the broken file raises one toast");
    assert_eq!(app.notices[0].file, config::CONFIG);
    let mut rt = runtime(app);
    rt.frame(0.1);
    assert!(
        rt.find("notice-config.kdl").is_some(),
        "the toast is on screen"
    );
}

#[test]
fn a_rule_the_schema_refuses_raises_a_toast_too() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "folder-rule \"x\" {\n    match path=\"/w\"\n    layout sideways\n}\n",
    )
    .unwrap();
    let (app, _path) = app_in_config_dir(&dir);
    assert_eq!(app.notices.len(), 1, "the refused rule raises one toast");
    assert!(
        app.notices[0].detail.contains("layout"),
        "the toast names the property: {}",
        app.notices[0].detail
    );
    assert!(
        app.notices[0].report.contains("what this file holds"),
        "and Copy hands over the schema: {}",
        app.notices[0].report
    );
}

#[test]
fn a_file_that_parses_again_takes_its_toast_away() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "settings {\n    grid #true\n}\n}\n",
    )
    .unwrap();
    let (mut app, _path) = app_in_config_dir(&dir);
    assert_eq!(app.notices.len(), 1);

    fs::write(
        dir.path().join("config.kdl"),
        "settings {\n    grid #true\n}\n",
    )
    .unwrap();
    app.store = config::Store::at(dir.path().to_path_buf());
    app.reload_rules();
    assert!(
        app.notices.is_empty(),
        "the fixed file takes its toast away"
    );
}

#[test]
fn a_second_read_of_the_same_problem_does_not_stack_a_toast() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        "settings {\n    grid #true\n}\n}\n",
    )
    .unwrap();
    let (mut app, _path) = app_in_config_dir(&dir);
    app.reload_rules();
    app.reload_rules();
    assert_eq!(
        app.notices.len(),
        1,
        "one toast per file, however often it is read"
    );
}
