//! The sidebar's places: the folders it can jump to, in the order the user left them, and the
//! machine-owned file that remembers it. Nothing here reads the folder on screen; a place is a
//! folder, a name and a glyph.
//!
//! The file keeps one block per section, each holding its places in display order:
//!
//! ```kdl
//! favourites {
//!     place "home"
//!     place "documents"
//!     place "folder:/home/me/Projects" name="Projects" path="/home/me/Projects"
//! }
//! locations {
//!     place "root"
//!     place "volume:1A2B-3C4D" name="Backup" path="/run/media/me/Backup"
//! }
//! ```
//!
//! A place is its `id`, never its position: the built-in ids (`home`, `desktop`, `documents`,
//! `downloads`, `pictures`, `root`, `temp`) follow the platform, `folder:<path>` is a folder of
//! the user's own, and `volume:<id>` is left for drives a later release detects. `path` says
//! where a place opens and is written only where the app cannot derive it; `name` overrides the
//! label. A built-in the file does not name is added at the end of its section, so a new one in
//! a later release still turns up. A place whose folder is not there keeps its place in the
//! file and comes back with the folder. Top-level nodes the app does not know are copied back
//! untouched, so a newer release's state survives an older one's reorder.

use crate::app::{home_dir, root_dir};
use crate::config;
use crate::icons;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use rune::Icon;
use rune_kit::glyphs;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The file under the state directory.
const FILE: &str = "places.kdl";

/// The two groups the sidebar draws, in the order it draws them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Favourites,
    Locations,
}

impl Section {
    pub const ALL: [Section; 2] = [Section::Favourites, Section::Locations];

    /// The section's heading, and the node that holds its places.
    pub fn title(self) -> &'static str {
        match self {
            Section::Favourites => "Favourites",
            Section::Locations => "Locations",
        }
    }

    fn node(self) -> &'static str {
        match self {
            Section::Favourites => "favourites",
            Section::Locations => "locations",
        }
    }

    fn list(self, places: &Places) -> &[Place] {
        match self {
            Section::Favourites => &places.favourites,
            Section::Locations => &places.locations,
        }
    }

    fn list_mut(self, places: &mut Places) -> &mut Vec<Place> {
        match self {
            Section::Favourites => &mut places.favourites,
            Section::Locations => &mut places.locations,
        }
    }
}

/// One row in the sidebar.
#[derive(Clone)]
pub struct Place {
    /// The stable name the state file keys on, never its position: a reorder changes the order,
    /// and a detected drive arrives with an id of its own.
    pub id: String,
    pub name: String,
    pub icon: &'static Icon,
    pub path: PathBuf,
}

impl Place {
    /// Whether the folder is there now. A place whose folder is not — an unmounted drive, a
    /// folder that was deleted — keeps its place in the file and comes back with the folder.
    pub fn is_available(&self) -> bool {
        self.path.is_dir()
    }
}

/// Where a built-in place opens on this platform.
enum Anchor {
    /// The home folder itself.
    Home,
    /// A folder under home, named the way the platform names it.
    UnderHome(&'static str),
    /// The root of the file system.
    Root,
    /// The system's temporary folder.
    Temp,
}

struct Builtin {
    id: &'static str,
    name: &'static str,
    icon: &'static Icon,
    section: Section,
    anchor: Anchor,
}

/// The places every launch knows about, in their starting order. The file names the ones the
/// user keeps and where they moved them; anything it leaves out is added at the end of its
/// section, so a place a later release adds still turns up.
const BUILTINS: [Builtin; 7] = [
    Builtin {
        id: "home",
        name: "Home",
        icon: &icons::HOUSE,
        section: Section::Favourites,
        anchor: Anchor::Home,
    },
    Builtin {
        id: "desktop",
        name: "Desktop",
        icon: &icons::DESKTOP,
        section: Section::Favourites,
        anchor: Anchor::UnderHome("Desktop"),
    },
    Builtin {
        id: "documents",
        name: "Documents",
        icon: &icons::DOCUMENTS,
        section: Section::Favourites,
        anchor: Anchor::UnderHome("Documents"),
    },
    Builtin {
        id: "downloads",
        name: "Downloads",
        icon: &icons::DOWNLOADS,
        section: Section::Favourites,
        anchor: Anchor::UnderHome("Downloads"),
    },
    Builtin {
        id: "pictures",
        name: "Pictures",
        icon: &icons::PICTURES,
        section: Section::Favourites,
        anchor: Anchor::UnderHome("Pictures"),
    },
    Builtin {
        id: "root",
        name: "Computer",
        icon: &icons::DRIVE,
        section: Section::Locations,
        anchor: Anchor::Root,
    },
    Builtin {
        id: "temp",
        name: "Temporary",
        icon: &glyphs::FOLDER,
        section: Section::Locations,
        anchor: Anchor::Temp,
    },
];

impl Builtin {
    fn path(&self) -> Option<PathBuf> {
        match self.anchor {
            Anchor::Home => home_dir(),
            Anchor::UnderHome(folder) => home_dir().map(|home| home.join(folder)),
            Anchor::Root => Some(root_dir()),
            Anchor::Temp => Some(std::env::temp_dir()),
        }
    }

    /// Whether the folder is there to show. The temporary folder is hidden when it is the root
    /// the root place already opens.
    fn exists(&self) -> bool {
        self.path().is_some_and(|path| path.is_dir())
            && !(self.id == "temp" && self.path() == Some(root_dir()))
    }

    fn place(&self) -> Option<Place> {
        Some(Place {
            id: self.id.to_owned(),
            name: self.name.to_owned(),
            icon: self.icon,
            path: self.path()?,
        })
    }
}

fn builtin(id: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|builtin| builtin.id == id)
}

/// The label for a place the app has no name for: the folder's own name.
fn label_for(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

/// The glyph for a place the app has no icon for. A volume a later release detects reads as a
/// drive; anything else reads as a folder.
fn icon_for(id: &str) -> &'static Icon {
    if id.starts_with("volume:") {
        &icons::DRIVE
    } else {
        &glyphs::FOLDER
    }
}

/// The sidebar's places and the file that remembers them.
#[derive(Default)]
pub struct Places {
    favourites: Vec<Place>,
    locations: Vec<Place>,
    /// The file, when the platform has a state folder to keep it in.
    path: Option<PathBuf>,
    /// Top-level nodes the app does not know, copied back untouched on the next save, so a
    /// newer release's state survives an older one's reorder.
    extra: Vec<KdlNode>,
    /// Why the file could not be read or written. Damaged state is reported, never fatal.
    pub error: Option<String>,
}

impl Places {
    /// The places saved for this user, with the built-ins the file does not mention.
    pub fn load() -> Places {
        let Some(dir) = config::state_dir() else {
            return Places::default();
        };
        Places::at(dir.join(FILE))
    }

    /// The places in `path`, for a state folder chosen by the caller.
    pub fn at(path: PathBuf) -> Places {
        match std::fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(parsed) => Places::build(Some(path), parsed),
                Err(err) => {
                    let mut places = Places::build(Some(path.clone()), Parsed::default());
                    places.error = Some(format!(
                        "Couldn’t read {}: {err}. The sidebar shows its default places.",
                        path.display()
                    ));
                    places
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Places::build(Some(path), Parsed::default())
            }
            Err(err) => {
                let mut places = Places::build(Some(path.clone()), Parsed::default());
                places.error = Some(format!("Couldn’t read {}: {err}", path.display()));
                places
            }
        }
    }

    /// One section's places, in display order. Some may not be on screen now.
    pub fn places(&self, section: Section) -> &[Place] {
        section.list(self)
    }

    /// Moves the `from`th place on screen to `to` among the places on screen, as a drop does.
    /// A place that is not on screen keeps the slot it has in the file, so a folder that comes
    /// back comes back where it was left. True when the order changed.
    pub fn reorder(&mut self, section: Section, from: usize, to: usize) -> bool {
        if from == to {
            return false;
        }
        let places = section.list_mut(self);
        let slots: Vec<usize> = places
            .iter()
            .enumerate()
            .filter(|(_, place)| place.is_available())
            .map(|(index, _)| index)
            .collect();
        if from >= slots.len() || to >= slots.len() {
            return false;
        }
        let mut shown: Vec<Place> = slots.iter().map(|&slot| places[slot].clone()).collect();
        let moved = shown.remove(from);
        shown.insert(to, moved);
        for (&slot, place) in slots.iter().zip(shown) {
            places[slot] = place;
        }
        true
    }

    /// Writes the file. A place whose folder is gone is written too: it is the file that
    /// remembers it until the folder comes back.
    pub fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        match config::write_atomic(&path, &render(self)) {
            Ok(()) => self.error = None,
            Err(err) => self.error = Some(err),
        }
    }

    /// The stored places in file order, then the built-ins the file leaves out.
    fn build(path: Option<PathBuf>, parsed: Parsed) -> Places {
        let mut places = Places {
            path,
            ..Places::default()
        };
        let Parsed {
            mut favourites,
            mut locations,
            extra,
        } = parsed;
        places.extra = extra;
        for section in Section::ALL {
            let stored = match section {
                Section::Favourites => &mut favourites,
                Section::Locations => &mut locations,
            };
            let mut seen = BTreeSet::new();
            let mut list = Vec::new();
            for entry in stored.drain(..) {
                if !seen.insert(entry.id.clone()) {
                    continue;
                }
                if let Some(place) = entry.resolve() {
                    list.push(place);
                }
            }
            for builtin in BUILTINS.iter().filter(|builtin| builtin.section == section) {
                if seen.contains(builtin.id) || !builtin.exists() {
                    continue;
                }
                if let Some(place) = builtin.place() {
                    list.push(place);
                }
            }
            *section.list_mut(&mut places) = list;
        }
        places
    }
}

/// One `place` as the file writes it: an id and whatever overrides the app would otherwise
/// derive.
struct Stored {
    id: String,
    path: Option<PathBuf>,
    name: Option<String>,
}

impl Stored {
    fn resolve(self) -> Option<Place> {
        if let Some(builtin) = builtin(&self.id) {
            let path = self.path.or_else(|| builtin.path())?;
            return Some(Place {
                id: self.id,
                name: self.name.unwrap_or_else(|| builtin.name.to_owned()),
                icon: builtin.icon,
                path,
            });
        }
        // A `folder:` place, a `volume:` one a later release detects, or an id this release
        // does not know: the path is all it takes to show it, so keep it in its place.
        let path = self.path?;
        Some(Place {
            name: self.name.unwrap_or_else(|| label_for(&path)),
            icon: icon_for(&self.id),
            id: self.id,
            path,
        })
    }
}

#[derive(Default)]
struct Parsed {
    favourites: Vec<Stored>,
    locations: Vec<Stored>,
    extra: Vec<KdlNode>,
}

fn parse(text: &str) -> Result<Parsed, String> {
    let doc: KdlDocument = text.parse().map_err(|err: kdl::KdlError| err.to_string())?;
    let mut parsed = Parsed::default();
    for node in doc.nodes() {
        match node.name().value() {
            "favourites" => parsed.favourites = stored(node),
            "locations" => parsed.locations = stored(node),
            // A node this release does not know, such as a later release's drive detection:
            // keep it whole and write it back.
            _ => parsed.extra.push(node.clone()),
        }
    }
    Ok(parsed)
}

/// The `place` children of one section, in file order. Anything else in the block is ignored,
/// so a later release's fields do not make this one drop the folder.
fn stored(node: &KdlNode) -> Vec<Stored> {
    node.children()
        .into_iter()
        .flat_map(|kids| kids.nodes())
        .filter(|child| child.name().value() == "place")
        .filter_map(|child| {
            let id = child.get(0).and_then(|entry| entry.as_string())?;
            let mut path = None;
            let mut name = None;
            for entry in child.entries() {
                let Some(key) = entry.name().map(|name| name.value()) else {
                    continue;
                };
                let value = entry.value().as_string().map(str::to_owned);
                match key {
                    "path" => path = value.map(PathBuf::from),
                    "name" => name = value,
                    _ => {}
                }
            }
            Some(Stored {
                id: id.to_owned(),
                path,
                name,
            })
        })
        .collect()
}

/// The file: the nodes it held that this release does not know, then one block per section in
/// the order the sidebar shows them.
fn render(places: &Places) -> String {
    let mut doc = KdlDocument::new();
    for node in &places.extra {
        doc.nodes_mut().push(node.clone());
    }
    for section in Section::ALL {
        let mut node = KdlNode::new(section.node());
        let mut children = KdlDocument::new();
        for place in section.list(places) {
            children.nodes_mut().push(place_node(place));
        }
        node.set_children(children);
        doc.nodes_mut().push(node);
    }
    doc.autoformat();
    format!(
        "// The sidebar's places, in the order they are shown. Sheath owns this file:\n\
         // a place is its id, and `path` and `name` override what the app derives.\n\
         {doc}"
    )
}

fn place_node(place: &Place) -> KdlNode {
    let mut node = KdlNode::new("place");
    node.push(KdlEntry::new(place.id.clone()));
    let builtin = builtin(&place.id);
    // Writing only what differs keeps the file readable, and a built-in keeps following the
    // machine's home folder instead of pinning the one it was saved on.
    let derived_path = builtin.and_then(|builtin| builtin.path());
    if derived_path.as_deref() != Some(place.path.as_path()) {
        node.push(KdlEntry::new_prop(
            "path",
            place.path.to_string_lossy().into_owned(),
        ));
    }
    if builtin.is_none_or(|builtin| builtin.name != place.name.as_str()) {
        node.push(KdlEntry::new_prop("name", place.name.clone()));
    }
    node
}
