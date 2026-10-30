//! The rule editor: the form, and what closes it.

use super::helpers::*;
use super::rules::open_rule_editor;
use std::fs;

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
