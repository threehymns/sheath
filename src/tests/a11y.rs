use super::helpers::*;
use crate::places::Places;
use rune::accesskit::{Node, Role, TreeUpdate};

fn node<'a>(tree: &'a TreeUpdate, key: &str) -> &'a Node {
    tree.nodes
        .iter()
        .find_map(|(_, node)| (node.author_id() == Some(key)).then_some(node))
        .unwrap_or_else(|| panic!("no accessibility node keyed {key:?}"))
}

#[test]
fn the_shell_reads_its_places_controls_and_current_folder() {
    let dir = fixture();
    let state = tempfile::tempdir().unwrap();
    let places_file = state.path().join("places.kdl");
    let id = format!("folder:{}", dir.path().display());
    write_places(
        &places_file,
        &[(id.clone(), dir.path().display().to_string())],
        "",
    );

    let mut app = app(&dir);
    app.places = Places::at(places_file);
    let mut rt = runtime(app);
    rt.frame(0.0);
    let tree = rt.accessibility_tree();

    // The list is named, and its two sections are named groups around their places.
    let list = node(&tree, "sidebar-list");
    assert_eq!(list.role(), Role::List);
    assert_eq!(list.label(), Some("Places"));
    for (key, title) in [
        ("places-favourites", "Favourites"),
        ("places-locations", "Locations"),
    ] {
        let section = node(&tree, key);
        assert_eq!(section.role(), Role::Group, "{key}");
        assert_eq!(section.label(), Some(title), "{key}");
    }

    // The place on screen reads its name and that it is the folder shown.
    let place = node(&tree, &format!("place-{id}"));
    assert_eq!(place.role(), Role::ListItem);
    assert_eq!(
        place.label(),
        dir.path().file_name().unwrap().to_str(),
        "the row says the name it shows"
    );
    assert_eq!(place.is_selected(), Some(true));

    // The path bar is the window's context, not an unnamed group of controls.
    let folder = format!("Current folder: {}", dir.path().display());
    let path = node(&tree, "path-bar");
    assert_eq!(path.role(), Role::Navigation);
    assert_eq!(path.label(), Some(folder.as_str()));

    // The toolbar's controls read by name.
    for (key, label) in [
        ("nav-back", "Back"),
        ("nav-forward", "Forward"),
        ("nav-up", "Enclosing Folder"),
        ("toggle-layout", "Grid Layout"),
    ] {
        assert_eq!(node(&tree, key).label(), Some(label), "{key}");
    }
    // A field's name is on the editor inside its box, as rune-kit lays it out.
    let search = tree
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| matches!(node.role(), Role::TextInput | Role::SearchInput))
        .expect("the search field is a text input");
    assert_eq!(search.label(), Some("Search this folder and below"));

    // The list's sort headers read their column's name.
    let name = node(&tree, "name");
    assert_eq!(name.role(), Role::ColumnHeader);
    assert_eq!(name.label(), Some("Name"));

    // The sidebar toggle says which way it will go.
    let toggle = node(&tree, "sidebar-toggle");
    assert_eq!(toggle.role(), Role::Button);
    assert_eq!(toggle.label(), Some("Hide sidebar"));
    assert_eq!(toggle.is_expanded(), Some(true));
}
