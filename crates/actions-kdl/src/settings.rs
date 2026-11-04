use actions::Setting;
use kdl::KdlDocument;

use crate::children::{remove_child, write_child};

/// The settings a document sets, with their values.
pub fn read<S: Setting>(doc: Option<&KdlDocument>) -> Vec<(S, bool)> {
    doc.map(parse).unwrap_or_default()
}

pub fn parse<S: Setting>(doc: &KdlDocument) -> Vec<(S, bool)> {
    let mut out = Vec::new();
    for node in doc.nodes() {
        if node.name().value() != "settings" {
            continue;
        }
        for child in node.children().into_iter().flat_map(|kids| kids.nodes()) {
            let Some(setting) = S::from_name(child.name().value()) else {
                continue;
            };
            if let Some(on) = child.get(0).and_then(|entry| entry.as_bool()) {
                out.push((setting, on));
            }
        }
    }
    out
}

pub fn write<S: Setting>(doc: &mut KdlDocument, setting: S, on: bool) {
    let body = format!("{} {}", setting.name(), if on { "#true" } else { "#false" });
    write_child(doc, "settings", setting.name(), &body);
}

pub fn remove<S: Setting>(doc: &mut KdlDocument, setting: S) {
    remove_child(doc, "settings", setting.name());
}
