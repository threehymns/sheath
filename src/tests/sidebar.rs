//! The sidebar's places: dragging them into a new order, and the state file that remembers it.

use super::helpers::*;
use crate::places::{Places, Section};
use rune::{Key, KeyEvent, Mods};
use std::fs;

/// A places file naming two folders under `dir`, in that order, plus whatever else the caller
/// wants at the top. The paths are quoted the way KDL writes strings.
fn write_places(path: &std::path::Path, rows: &[(String, String)], extra: &str) {
    let mut text = String::from(extra);
    text.push_str("favourites {\n");
    for (id, folder) in rows {
        text.push_str(&format!(
            "    place {} name={} path={}\n",
            crate::rules::quote(id),
            crate::rules::quote(folder.rsplit('/').next().unwrap_or(folder)),
            crate::rules::quote(folder),
        ));
    }
    text.push_str("}\n");
    fs::write(path, text).unwrap();
}

#[test]
fn dragging_a_place_down_moves_it_and_writes_the_file() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let second_id = format!("folder:{}", second.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[
            (alpha_id.clone(), alpha.display().to_string()),
            (second_id.clone(), second.display().to_string()),
        ],
        "",
    );
    assert!(!file.exists() || fs::read_to_string(&file).unwrap().contains(&alpha_id));

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let alpha_key = format!("place-{alpha_id}");
    let second_key = format!("place-{second_id}");
    let (x, y) = rt.find(&alpha_key).expect("the first place is on screen");
    let (_, below) = rt.find(&second_key).expect("the second place is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.02);
    rt.pointer_move(x, below);
    // The rows step aside on a spring, so let the gap finish opening.
    for _ in 0..10 {
        rt.frame(0.05);
    }
    let (_, lifted) = rt
        .find(&alpha_key)
        .expect("the row in hand is still on screen");
    assert!(
        lifted > y + 20.0,
        "the row in hand follows the pointer: {lifted} from {y}"
    );

    rt.pointer_up(x, below);
    rt.frame(0.05);

    let shown: Vec<&str> = rt
        .state
        .places
        .places(Section::Favourites)
        .iter()
        .map(|place| place.id.as_str())
        .take(2)
        .collect();
    assert_eq!(
        shown,
        [second_id.as_str(), alpha_id.as_str()],
        "the drop moved the place below its neighbour"
    );
    let text = fs::read_to_string(&file).unwrap();
    let second_at = text
        .find(&crate::rules::quote(&second_id))
        .expect("the second place is in the file");
    let alpha_at = text
        .find(&crate::rules::quote(&alpha_id))
        .expect("the dragged place is still in the file");
    assert!(second_at < alpha_at, "the file holds the new order: {text}");
    assert_eq!(
        rt.state.cwd.as_path(),
        dir.path(),
        "the release that ended the drag did not open the dragged place"
    );
    assert!(
        !rt.state.place_dragged,
        "the release consumed the drag, so it does not eat the next press"
    );
}

#[test]
fn a_place_that_is_not_on_screen_keeps_its_slot_and_unknown_nodes_survive() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let alpha = dir.path().join("alpha");
    let missing = dir.path().join("unplugged");
    let alpha_id = format!("folder:{}", alpha.display());
    let second_id = format!("folder:{}", second.display());
    let missing_id = format!("folder:{}", missing.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[
            (alpha_id.clone(), alpha.display().to_string()),
            (missing_id.clone(), missing.display().to_string()),
            (second_id.clone(), second.display().to_string()),
        ],
        // A node a later release writes, such as its drive detection: this release keeps it.
        "drives {\n    detect-external-drives #true\n}\n",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let alpha_key = format!("place-{alpha_id}");
    let second_key = format!("place-{second_id}");
    assert!(
        rt.find(&format!("place-{missing_id}")).is_none(),
        "the place whose folder is not there is not drawn"
    );
    let (x, y) = rt.find(&alpha_key).expect("the first place is on screen");
    let (_, below) = rt.find(&second_key).expect("the last place is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.02);
    rt.pointer_move(x, below);
    rt.frame(0.05);
    rt.pointer_up(x, below);
    rt.frame(0.05);

    let text = fs::read_to_string(&file).unwrap();
    let second_at = text
        .find(&crate::rules::quote(&second_id))
        .expect("the second place is in the file");
    let missing_at = text
        .find(&crate::rules::quote(&missing_id))
        .expect("the missing place keeps its row in the file");
    let alpha_at = text
        .find(&crate::rules::quote(&alpha_id))
        .expect("the dragged place is still in the file");
    assert!(
        second_at < missing_at && missing_at < alpha_at,
        "the file holds the new order around the missing place: {text}"
    );
    assert!(
        text.contains("detect-external-drives #true"),
        "a node this release does not know survives the write: {text}"
    );
}

#[test]
fn dropping_a_folder_on_a_section_adds_it_as_a_place() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let file = state.path().join("places.kdl");
    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let source = dir.path().join("alpha");
    click(&mut rt, "f:alpha");
    let (x0, y0) = rt.find("f:alpha").expect("the folder row is on screen");
    let (section, _) = rt
        .presented("places-favourites")
        .expect("the favourites section is drawn");
    // The heading band above the rows: a drop there adds, where a drop on a row moves.
    let (x1, y1) = (section[0] + 40.0, section[1] + 12.0);
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x1, y1);
    rt.frame(0.05);
    assert_eq!(
        rt.state.drop_place(),
        None,
        "the heading is not one of the place rows"
    );
    rt.pointer_up(x1, y1);
    rt.frame(0.1);

    assert!(source.is_dir(), "the drop did not move the folder");
    assert!(
        rt.state
            .places
            .places(Section::Favourites)
            .iter()
            .any(|place| place.path == source),
        "the folder is a favourite now"
    );
    let text = fs::read_to_string(&file).expect("the drop wrote the state file");
    assert!(
        text.contains(&crate::rules::quote(&format!(
            "folder:{}",
            source.display()
        ))),
        "the file names the folder: {text}"
    );
}

#[test]
fn dropping_between_places_inserts_there_with_a_preview() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox");
    fs::create_dir(&inbox).unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let second_id = format!("folder:{}", second.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[
            (alpha_id.clone(), alpha.display().to_string()),
            (second_id.clone(), second.display().to_string()),
        ],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let (alpha_row, _) = rt
        .presented(&format!("place-{alpha_id}"))
        .expect("the first place is on screen");
    let (second_row, _) = rt
        .presented(&format!("place-{second_id}"))
        .expect("the second place is on screen");
    // The two point gap between the rows, where an item dropped lands in the order.
    let (x, y) = (alpha_row[0] + 40.0, alpha_row[1] + alpha_row[3] + 1.0);

    let (x0, y0) = rt.find("f:inbox").expect("the folder row is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x, y);
    rt.frame(0.05);
    rt.frame(0.05);

    assert_eq!(
        rt.state.place_insert(),
        Some((Section::Favourites, 1)),
        "the pointer sits between the first and second place"
    );
    let (line, _) = rt
        .presented("place-insert")
        .expect("the insertion line marks where the rows would land");
    assert!(
        line[3] <= 2.5,
        "the line takes up no room of its own: {line:?}"
    );
    assert!(
        rt.find("drag-badge").is_some(),
        "the item itself previews under the pointer"
    );
    let (still, _) = rt
        .presented(&format!("place-{second_id}"))
        .expect("the second place is still drawn");
    assert!(
        (still[1] - second_row[1]).abs() < 0.5,
        "the rows stay where they are: {still:?} from {second_row:?}"
    );

    rt.pointer_up(x, y);
    rt.frame(0.1);

    let inbox_id = format!("folder:{}", inbox.display());
    let shown: Vec<&str> = rt
        .state
        .places
        .places(Section::Favourites)
        .iter()
        .map(|place| place.id.as_str())
        .take(3)
        .collect();
    assert_eq!(
        shown,
        [alpha_id.as_str(), inbox_id.as_str(), second_id.as_str()],
        "the drop inserted the folder between the two places"
    );
    let text = fs::read_to_string(&file).unwrap();
    let inbox_at = text
        .find(&crate::rules::quote(&inbox_id))
        .expect("the file names the new place");
    let second_at = text
        .find(&crate::rules::quote(&second_id))
        .expect("the second place is still in the file");
    assert!(
        inbox_at < second_at,
        "the file holds the new place before the second: {text}"
    );
    assert!(inbox.is_dir(), "the drop did not move the folder");
}

#[test]
fn a_file_place_opens_its_folder_with_the_file_selected() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let file = state.path().join("places.kdl");
    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let notes = dir.path().join("notes.md");
    let (x0, y0) = rt.find("f:notes.md").expect("the file row is on screen");
    let (section, _) = rt
        .presented("places-favourites")
        .expect("the favourites section is drawn");
    let (x1, y1) = (section[0] + 40.0, section[1] + 12.0);
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x1, y1);
    rt.frame(0.05);
    rt.pointer_up(x1, y1);
    rt.frame(0.1);
    assert!(notes.is_file(), "the file did not move");

    // Pick another row, so the file being selected afterwards says the place did it.
    click(&mut rt, "f:file2.txt");
    let key = format!("place-file:{}", notes.display());
    let (x, y) = rt.find(&key).expect("the file is a place now");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    rt.frame(0.1);

    assert_eq!(
        rt.state.cwd.as_path(),
        dir.path(),
        "the place opened the folder holding the file"
    );
    assert_eq!(
        rt.state.selected.as_deref(),
        Some("f:notes.md"),
        "the file is selected"
    );
}

#[test]
fn right_clicking_a_place_offers_a_menu_and_removing_hides_its_row() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let second_id = format!("folder:{}", second.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[
            (alpha_id.clone(), alpha.display().to_string()),
            (second_id.clone(), second.display().to_string()),
        ],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    let key = format!("place-{alpha_id}");
    right_click(&mut rt, &key);
    assert!(rt.state.place_menu.is_some(), "the place menu opens");
    assert!(rt.find("place-menu-item0").is_some(), "Open is offered");
    assert!(
        rt.find("place-menu-item1").is_some(),
        "Show in Folder is offered"
    );
    assert!(rt.find("place-menu-item3").is_some(), "Remove is offered");

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.place_menu.is_none(), "Escape closes the menu");
    assert!(rt.find(&key).is_some(), "and leaves the place alone");

    right_click(&mut rt, &key);
    click(&mut rt, "place-menu-item3");
    rt.frame(0.1);

    assert!(
        rt.state.place_menu.is_none(),
        "picking an item closes the menu"
    );
    assert!(rt.find(&key).is_none(), "the row leaves the sidebar");
    assert!(
        rt.state
            .places
            .get(&alpha_id)
            .is_some_and(|place| place.hidden),
        "the place is kept in the file, hidden"
    );
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("hidden=#true"),
        "the file marks it hidden: {text}"
    );
    assert!(
        rt.find(&format!("place-{second_id}")).is_some(),
        "the other place stays"
    );
}

#[test]
fn the_place_menu_shows_a_place_in_its_folder() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let second = dir.path().join("second");
    fs::create_dir(&second).unwrap();
    let alpha = dir.path().join("alpha");
    let alpha_id = format!("folder:{}", alpha.display());
    let file = state.path().join("places.kdl");
    write_places(
        &file,
        &[(alpha_id.clone(), alpha.display().to_string())],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(file.clone());
    let mut rt = runtime(app);
    rt.frame(0.0);

    // Move away, so showing the place has to bring its folder back.
    rt.state.navigate(second.clone());
    rt.frame(0.1);
    assert_eq!(rt.state.cwd.as_path(), second.as_path());

    right_click(&mut rt, &format!("place-{alpha_id}"));
    click(&mut rt, "place-menu-item1");
    rt.frame(0.1);
    rt.frame(0.1);

    assert_eq!(
        rt.state.cwd.as_path(),
        dir.path(),
        "the folder holding the place is open"
    );
    assert_eq!(
        rt.state.selected.as_deref(),
        Some("f:alpha"),
        "the place is selected there"
    );
}

#[test]
fn a_place_from_a_later_release_keeps_its_id_and_label() {
    let state = tempfile::tempdir().unwrap();
    let file = state.path().join("places.kdl");
    fs::write(
        &file,
        "favourites {\n    place \"volume:1A2B-3C4D\" name=\"Backup\" path=\"/run/media/backup\"\n}\n",
    )
    .unwrap();

    let places = Places::at(file);
    let first = &places.places(Section::Favourites)[0];
    assert_eq!(first.id, "volume:1A2B-3C4D");
    assert_eq!(first.name, "Backup");
    assert!(
        !first.is_available(),
        "an unplugged drive is remembered but not drawn"
    );
}
