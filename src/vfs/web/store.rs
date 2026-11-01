//! Sheath's own KDL files, and the OPFS behind them.
//!
//! Settings, keybinds, views, session and places live in the demo home, but between visits OPFS
//! keeps them: `hydrate` reads them before the window opens, and every write to one of them goes
//! back out to the browser.

use super::browser;
use super::{CONFIG_DIR, STATE_DIR, with};
use std::path::Path;

/// Sheath's own files, which OPFS keeps between visits.
const PERSISTED: [&str; 5] = [
    "config.kdl",
    "keybinds.kdl",
    "views.kdl",
    "session.kdl",
    "places.kdl",
];

fn persisted_dir(name: &str) -> &'static str {
    match name {
        "config.kdl" | "keybinds.kdl" => CONFIG_DIR,
        _ => STATE_DIR,
    }
}

/// Reads the KDL files OPFS kept from the last visit into the tree, before the window opens.
pub async fn hydrate() {
    let names: Vec<&str> = PERSISTED.to_vec();
    let texts = browser::load(&names).await;
    for (name, text) in PERSISTED.iter().zip(texts) {
        if let Some(text) = text {
            let path = Path::new(persisted_dir(name)).join(name);
            with(|tree| tree.put(path, text.into_bytes()));
        }
    }
}

/// The OPFS name a path saves under, when it is one of Sheath's own files.
fn opfs_name(path: &Path) -> Option<String> {
    let parent = path.parent()?;
    if parent != Path::new(CONFIG_DIR) && parent != Path::new(STATE_DIR) {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    (!name.starts_with('.')).then(|| name.to_owned())
}

/// Saves a copy of one of Sheath's own files, so the next visit starts where this one left off.
pub(super) fn persist(path: &Path, content: &[u8]) {
    let Some(name) = opfs_name(path) else {
        return;
    };
    if let Ok(text) = std::str::from_utf8(content) {
        browser::save(&name, text);
    }
}

/// Drops the saved copy of a file the tree no longer holds.
pub(super) fn forget(path: &Path) {
    if let Some(name) = opfs_name(path) {
        browser::remove(&name);
    }
}
