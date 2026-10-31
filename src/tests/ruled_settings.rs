use super::helpers::*;
use crate::app::App;
use std::fs;

#[test]
fn the_grid_switch_leaves_a_ruled_folder_alone() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "folder-rule \"here\" {{\n    match path=\"{}\"\n    layout grid\n}}\n",
            dir.path().display()
        ),
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(rt.state.is_grid());

    click(&mut rt, "open-settings");
    click(&mut rt, "setting-grid");
    assert!(
        rt.state.is_grid(),
        "the rule still decides the layout after the switch"
    );
    assert!(saved_settings(&dir).grid, "the setting itself is written");
}

#[test]
fn the_hidden_switch_writes_the_global_setting() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    click(&mut rt, "setting-hidden");
    assert!(rt.state.show_hidden, "the switch shows the dot files");
    assert!(
        saved_settings(&dir).show_hidden,
        "the setting is written to config.kdl"
    );
}

#[test]
fn the_hidden_switch_leaves_a_ruled_folder_alone() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "folder-rule \"here\" {{\n    match path=\"{}\"\n    show-hidden #true\n}}\n",
            dir.path().display()
        ),
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(rt.state.show_hidden);

    click(&mut rt, "open-settings");
    click(&mut rt, "setting-hidden");
    assert!(
        rt.state.show_hidden,
        "the rule still decides the hidden files after the switch"
    );
    assert!(
        saved_settings(&dir).show_hidden,
        "the setting itself is written"
    );
}

#[test]
fn a_ruled_folders_layout_does_not_become_the_global_default() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "folder-rule \"here\" {{\n    match path=\"{}\"\n    layout grid\n    show-hidden #true\n}}\n",
            dir.path().display()
        ),
    )
    .unwrap();

    let (mut app, _path) = app_in_config_dir(&dir);
    assert!(app.is_grid() && app.show_hidden, "the rule holds here");

    app.toggle_inspector();
    app.navigate(dir.path().join("alpha"));
    assert!(!app.is_grid(), "the rule's layout stays with its folder");
    assert!(
        !app.show_hidden,
        "the rule's hidden policy stays with its folder"
    );
}

#[test]
fn a_ruled_folder_still_leaves_the_general_settings_editable() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "folder-rule \"here\" {{\n    match path=\"{}\"\n    layout grid\n    show-hidden #true\n}}\n",
            dir.path().display()
        ),
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(
        rt.state.is_grid() && rt.state.show_hidden,
        "the rule holds here"
    );
    click(&mut rt, "open-settings");

    for key in ["setting-grid", "setting-hidden"] {
        let thumb = thumb_off(&rt, key);
        assert!(
            thumb,
            "{key} shows the fallback, which is off, not the rule"
        );
    }

    click(&mut rt, "setting-grid");
    assert!(rt.state.config.grid, "the switch sets the global setting");
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
    assert!(
        !thumb_off(&rt, "setting-grid"),
        "the switch shows the fallback it just set"
    );

    click(&mut rt, "setting-hidden");
    assert!(rt.state.config.show_hidden, "the other switch sets its own");
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
    assert!(
        !thumb_off(&rt, "setting-hidden"),
        "and shows the value it set"
    );
    assert!(
        rt.state.is_grid() && rt.state.show_hidden,
        "the rule still decides the folder"
    );
}

fn thumb_off(rt: &rune::Runtime<App>, key: &str) -> bool {
    let elements = rt.elements();
    let track = elements
        .iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("{key} is drawn"));
    let thumb = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("thumb") && e.path.starts_with(&track.path))
        .unwrap_or_else(|| panic!("{key} draws a thumb"));
    let (track, thumb) = (track.rect, thumb.rect);
    thumb[0] + thumb[2] / 2.0 < track[0] + track[2] / 2.0
}
