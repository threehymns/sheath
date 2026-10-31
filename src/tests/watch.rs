use super::helpers::*;
use rune::WheelEvent;
use std::fs;

/// The inotify instances this process holds open; a count that keeps climbing means leaked
/// watchers.
#[cfg(target_os = "linux")]
fn inotify_instances() -> usize {
    let Ok(fds) = fs::read_dir("/proc/self/fd") else {
        return 0;
    };
    fds.flatten()
        .filter(|fd| {
            fs::read_link(fd.path())
                .map(|target| target.to_string_lossy().contains("inotify"))
                .unwrap_or(false)
        })
        .count()
}

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

// A watch update above the reading position must not push the rows the reader is on.
#[test]
fn a_file_added_above_the_view_leaves_the_rows_where_they_are() {
    let dir = long_folder(60);
    let mut app = app(&dir);
    app.config.scroll_inertia = false;
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    app.watch(wake);
    let mut rt = runtime(app);
    rt.frame(0.0);
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    for _ in 0..3 {
        rt.wheel_event(WheelEvent::new(0.0, -120.0).at(x, y));
        rt.frame(1.0 / 60.0);
    }
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let scrolled = scroll_offset(&rt);
    assert!(scrolled > 100.0, "the list is scrolled down: {scrolled}");
    let row = rt
        .find("f:file20.txt")
        .expect("a row below the top is on screen")
        .1;

    fs::write(dir.path().join("000-arrived.txt"), "x").unwrap();
    let mut landed = false;
    for _ in 0..100 {
        rt.frame(0.1);
        if shown(&rt, "000-arrived.txt") {
            landed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(landed, "the watcher landed a file above the view");
    let after = rt
        .find("f:file20.txt")
        .expect("the same row is still on screen")
        .1;
    assert!(
        (after - row).abs() < 1.0,
        "the row the reader was on stayed put: {row} -> {after}"
    );
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

    fs::write(dir.path().join("left-behind.txt"), "x").unwrap();
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

#[cfg(target_os = "linux")]
#[test]
fn switching_folders_leaves_no_watcher_behind() {
    let dir = fixture();
    let other = tempfile::tempdir().unwrap();
    let mut app = app(&dir);
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    app.watch(wake);
    app.settle_scan();
    let first = inotify_instances();
    assert!(first > 0, "the folder is watched to begin with");

    for _ in 0..20 {
        app.navigate(other.path().to_path_buf());
        app.settle_scan();
        app.navigate(dir.path().to_path_buf());
        app.settle_scan();
    }
    assert_eq!(
        app.watchers(),
        1,
        "the app holds one watcher after twenty switches, not one per visit"
    );
    // Other tests run in parallel and open their own watchers, so the process-wide count is only
    // meaningful as a bound: a leak of one per switch would climb by forty, not by a few.
    let mut left = inotify_instances();
    for _ in 0..100 {
        if left <= first {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        left = inotify_instances();
    }
    assert!(
        left < first + 10,
        "twenty switches hold one watcher, not twenty: {left} against {first}"
    );
}

// A preview opens its picture, and listing a folder reads it: neither may count as a change,
// or the reload that follows would open the same file again, in a loop.
#[test]
fn opening_a_file_is_not_a_change_to_its_folder() {
    use notify::event::{AccessKind, AccessMode, DataChange, ModifyKind};
    use notify::{Event, EventKind};
    use std::path::Path;

    let dir = Path::new("/tmp/photos");
    let access = Event::new(EventKind::Access(AccessKind::Open(AccessMode::Any)))
        .add_path(dir.join("photo.jpg"));
    assert!(
        !crate::app::changed(&access, dir),
        "opening a file to read it is not a change"
    );

    let written = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
        .add_path(dir.join("photo.jpg"));
    assert!(
        crate::app::changed(&written, dir),
        "writing a file is a change"
    );

    let elsewhere = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
        .add_path(Path::new("/tmp/other/photo.jpg").to_path_buf());
    assert!(
        !crate::app::changed(&elsewhere, dir),
        "another folder's files are not this folder's change"
    );
}
