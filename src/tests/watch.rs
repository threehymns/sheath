//! The open folder watched for changes made from outside the app.

use super::helpers::*;
use std::fs;

/// Whether a row named `name` is in the list on screen.
fn shown(rt: &rune::Runtime<crate::app::App>, name: &str) -> bool {
    rt.state
        .visible
        .iter()
        .any(|&i| rt.state.entries[i].name == name)
}

#[test]
fn a_file_added_by_someone_else_lands_in_the_list() {
    let dir = fixture();
    let mut app = app(&dir);
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    app.watch(wake);
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(!shown(&rt, "dropped.txt"), "the folder starts without it");

    fs::write(dir.path().join("dropped.txt"), "x").unwrap();
    let mut landed = false;
    for _ in 0..100 {
        rt.frame(0.1);
        if shown(&rt, "dropped.txt") {
            landed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(landed, "the watcher reloaded the folder the file landed in");
}

#[test]
fn a_file_removed_by_someone_else_leaves_the_list() {
    let dir = fixture();
    let mut app = app(&dir);
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    app.watch(wake);
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(shown(&rt, "notes.md"), "the folder starts with it");

    fs::remove_file(dir.path().join("notes.md")).unwrap();
    let mut gone = false;
    for _ in 0..100 {
        rt.frame(0.1);
        if !shown(&rt, "notes.md") {
            gone = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(gone, "the watcher dropped the row the file left behind");
}

#[test]
fn the_watcher_follows_the_window_to_another_folder() {
    let dir = fixture();
    let other = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    app.watch(wake);
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.navigate(other.path().to_path_buf());
    rt.frame(0.1);

    // A change in the folder the app left is not this list's business.
    fs::write(dir.path().join("left-behind.txt"), "x").unwrap();
    // One in the folder now on screen is.
    fs::write(other.path().join("arrived.txt"), "x").unwrap();
    let mut landed = false;
    for _ in 0..100 {
        rt.frame(0.1);
        if shown(&rt, "arrived.txt") {
            landed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(landed, "the new folder is watched");
    assert!(
        !shown(&rt, "left-behind.txt"),
        "the folder left behind is not on screen"
    );
}
