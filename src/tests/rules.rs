//! Rules: the matchers, the order they apply in, the view they resolve to, and the saves that
//! keep the file they live in intact.

use super::helpers::*;
use crate::app::ViewMode;
use crate::config::{self, Store};
use crate::rules::Rules;
use crate::settings::{Setting, Settings};
use rune_kit::SortDir;
use std::fs;
use std::path::Path;

fn rules(text: &str) -> Rules {
    let doc: kdl::KdlDocument = text.parse().expect("the test's KDL parses");
    Rules::parse(&doc).expect("the test's rules parse")
}

#[test]
fn a_rule_covers_the_folder_and_everything_under_it() {
    let rules = rules(
        "folder-rule \"tree\" {\n    match path=\"/work/**\"\n    view grid\n}\n",
    );
    for path in ["/work", "/work/projects", "/work/projects/deep"] {
        assert_eq!(
            rules.resolve(Path::new(path), 0).view,
            Some(ViewMode::Grid),
            "{path} is under the tree"
        );
    }
    assert_eq!(rules.resolve(Path::new("/workshop"), 0).view, None);
    assert_eq!(rules.resolve(Path::new("/elsewhere"), 0).view, None);
}

#[test]
fn matchers_or_together_and_keys_inside_one_and_together() {
    let rules = rules(
        "folder-rule \"either\" {\n\
         \x20   match path=\"/a\"\n\
         \x20   match path=\"/b\"\n\
         \x20   view grid\n\
         }\n\
         folder-rule \"both\" {\n\
         \x20   match path=\"/c\" min-items=3\n\
         \x20   view list\n\
         }\n",
    );
    assert_eq!(rules.resolve(Path::new("/a"), 0).view, Some(ViewMode::Grid));
    assert_eq!(rules.resolve(Path::new("/b"), 0).view, Some(ViewMode::Grid));
    assert_eq!(rules.resolve(Path::new("/c"), 3).view, Some(ViewMode::List));
    assert_eq!(
        rules.resolve(Path::new("/c"), 2).view,
        None,
        "the count has to hold too"
    );
}

#[test]
fn an_exclude_vetoes_a_rule_that_would_otherwise_match() {
    let rules = rules(
        "folder-rule \"all but one\" {\n\
         \x20   match path=\"/w/**\"\n\
         \x20   exclude path=\"/w/skip\"\n\
         \x20   view grid\n\
         }\n",
    );
    assert_eq!(rules.resolve(Path::new("/w"), 0).view, Some(ViewMode::Grid));
    assert_eq!(rules.resolve(Path::new("/w/skip"), 0).view, None);
}

#[test]
fn a_later_rule_wins_property_by_property_and_inherit_undoes_it() {
    let rules = rules(
        "folder-rule \"base\" {\n\
         \x20   match path=\"/w\"\n\
         \x20   view grid\n\
         \x20   sort modified\n\
         \x20   direction descending\n\
         }\n\
         folder-rule \"narrow\" {\n\
         \x20   match path=\"/w\"\n\
         \x20   sort inherit\n\
         \x20   view list\n\
         }\n",
    );
    let resolved = rules.resolve(Path::new("/w"), 0);
    assert_eq!(resolved.view, Some(ViewMode::List));
    assert_eq!(resolved.sort, None, "inherit cleared what the base set");
    assert_eq!(resolved.direction, Some(SortDir::Descending));
}

#[test]
fn an_entry_rule_speaks_for_one_row() {
    let rules = rules(
        "entry-rule \"config\" {\n\
         \x20   match path=\"/home/u/.config\"\n\
         \x20   hidden #false\n\
         }\n\
         entry-rule \"junk\" {\n\
         \x20   match name=\"*.log\"\n\
         \x20   hidden #true\n\
         }\n",
    );
    let config_entry = crate::rules::EntryRef {
        path: "/home/u/.config",
        name: ".config",
        is_dir: true,
    };
    assert_eq!(rules.hidden(&config_entry), Some(false));
    let junk = crate::rules::EntryRef {
        path: "/home/u/a.log",
        name: "a.log",
        is_dir: false,
    };
    assert_eq!(rules.hidden(&junk), Some(true));
    let other = crate::rules::EntryRef {
        path: "/home/u/a.txt",
        name: "a.txt",
        is_dir: false,
    };
    assert_eq!(rules.hidden(&other), None);
}

#[test]
fn an_unknown_property_names_the_ones_that_work() {
    let doc: kdl::KdlDocument = "folder-rule \"x\" {\n    icon \"rust\"\n}\n"
        .parse()
        .unwrap();
    let error = Rules::parse(&doc).expect_err("icon is not in the schema yet");
    assert!(error.contains("unknown property `icon`"), "{error}");
    assert!(error.contains("view"), "{error}");
    assert!(error.contains("show-hidden"), "{error}");
}

#[test]
fn saving_a_setting_keeps_comments_and_the_rest_of_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.kdl");
    fs::write(
        &path,
        "// my settings\nsettings {\n    // keep me\n    show-hidden #false\n}\n\n\
         // the tree\nfolder-rule \"tree\" {\n    match path=\"/w/**\"\n    view grid\n}\n",
    )
    .unwrap();

    let mut store = Store::at(dir.path().to_path_buf());
    let mut settings = Settings::defaults();
    settings.show_hidden = true;
    store
        .edit(config::CONFIG, |doc| {
            settings.write(doc, Setting::ShowHidden)
        })
        .expect("the store writes");

    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("// my settings"), "{text}");
    assert!(text.contains("// keep me"), "{text}");
    assert!(text.contains("// the tree"), "{text}");
    assert!(text.contains("show-hidden #true"), "{text}");
    assert!(text.contains("folder-rule"), "{text}");
}

#[test]
fn a_file_that_does_not_parse_is_never_written_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.kdl");
    fs::write(&path, "settings {\n    show-hidden #true\n").unwrap();

    let mut store = Store::at(dir.path().to_path_buf());
    assert!(
        store
            .error(config::CONFIG)
            .is_some_and(|error| error.contains("Couldn’t read")),
        "{:?}",
        store.error(config::CONFIG)
    );
    assert!(
        store
            .edit(config::CONFIG, |doc| doc.nodes_mut().clear())
            .is_err(),
        "the store refuses to write a document it could not read"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "settings {\n    show-hidden #true\n",
        "the file is untouched"
    );
}

#[test]
fn a_folder_rule_decides_the_view_a_folder_opens_in() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "folder-rule \"here\" {{\n    match path=\"{}\"\n    view grid\n}}\n",
            dir.path().display()
        ),
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    assert!(app.is_grid(), "the rule opened the folder in the grid");
}

#[test]
fn an_entry_rule_reveals_a_hidden_folder() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "entry-rule \"secret\" {{\n    match path=\"{}\"\n    hidden #false\n}}\n",
            dir.path().join(".secret").display()
        ),
    )
    .unwrap();

    let (app, _path) = app_in_config_dir(&dir);
    assert!(
        names(&app).contains(&".secret"),
        "the entry rule shows a dot folder: {:?}",
        names(&app)
    );
    assert!(
        names(&app).contains(&"file2.txt"),
        "the rest of the folder is unchanged"
    );
}

#[test]
fn saving_the_view_as_a_rule_writes_one_that_applies() {
    let dir = fixture();
    let (mut app, _path) = app_in_config_dir(&dir);
    app.view = ViewMode::Grid;
    app.save_view_rule();

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("folder-rule"), "{text}");
    assert!(text.contains("view grid"), "{text}");
    assert!(!app.rules.rules.is_empty(), "the rule is live");
    assert!(app.is_grid());
}

#[test]
fn pinning_an_entry_writes_an_entry_rule_that_applies() {
    let dir = fixture();
    let (mut app, _path) = app_in_config_dir(&dir);
    app.select_id("f:.secret");
    app.pin_entry(false);
    app.reload();

    let text = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(text.contains("entry-rule"), "{text}");
    assert!(text.contains("hidden #false"), "{text}");
    assert!(names(&app).contains(&".secret"), "the pinned entry shows");
}

#[test]
fn a_toolbar_change_in_a_ruled_folder_lasts_for_the_visit() {
    let dir = fixture();
    fs::write(
        dir.path().join("config.kdl"),
        format!(
            "folder-rule \"here\" {{\n    match path=\"{}\"\n    view grid\n}}\n",
            dir.path().display()
        ),
    )
    .unwrap();

    let (mut app, _path) = app_in_config_dir(&dir);
    assert!(app.is_grid());
    app.toggle_view();
    assert!(!app.is_grid(), "the toolbar overrides for the visit");

    let here = app.cwd.clone();
    let other = dir.path().join("alpha");
    app.navigate(other);
    app.back();
    assert!(app.cwd == here);
    assert!(app.is_grid(), "the rule wins again on the next visit");
}
