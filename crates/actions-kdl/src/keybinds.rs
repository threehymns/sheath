use actions::{Action, Keybinds, Shortcut, default_shortcut};
use kdl::KdlDocument;

use crate::children::{remove_child, write_child};

/// A keybind table from a document, with the app's defaults under it.
pub fn read<A: Action>(doc: Option<&KdlDocument>) -> Keybinds<A> {
    match doc {
        Some(doc) => Keybinds::from_entries(parse(doc)),
        None => Keybinds::defaults(),
    }
}

/// The bindings a document names, in file order. An empty chord clears the shortcut.
pub fn parse<A: Action>(doc: &KdlDocument) -> Vec<(A, Option<Shortcut>)> {
    let mut out = Vec::new();
    for node in doc.nodes() {
        if node.name().value() != "keybinds" {
            continue;
        }
        for child in node.children().into_iter().flat_map(|kids| kids.nodes()) {
            let Some(action) = A::from_id(child.name().value()) else {
                continue;
            };
            let raw = child
                .get(0)
                .and_then(|value| value.as_string())
                .unwrap_or_default()
                .trim();
            let chord = if raw.is_empty() {
                None
            } else {
                match raw.parse::<Shortcut>() {
                    Ok(chord) => Some(chord),
                    Err(_) => continue,
                }
            };
            out.push((action, chord));
        }
    }
    out
}

/// Writes one action's chord, or takes its line out when it is the default again.
pub fn set<A: Action>(doc: &mut KdlDocument, keybinds: &Keybinds<A>, action: A) {
    let current = keybinds.chord(action);
    if current == default_shortcut(action).as_ref() {
        remove_child(doc, "keybinds", action.id());
        return;
    }
    let text = match current {
        Some(chord) => format!("{} \"{chord}\"", action.id()),
        None => format!("{} \"\"", action.id()),
    };
    write_child(doc, "keybinds", action.id(), &text);
}
