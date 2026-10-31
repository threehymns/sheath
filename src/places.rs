//! The sidebar's places, and the machine-owned file that remembers their order.
//!
//! ```kdl
//! favourites {
//!     place "home"
//!     place "folder:/home/me/Projects" name="Projects" path="/home/me/Projects"
//! }
//! locations {
//!     place "root"
//!     place "volume:1A2B-3C4D" name="Backup" path="/run/media/me/Backup"
//! }
//! ```
//!
//! A place is its `id`, never its position. `path` is written only where the app cannot derive
//! it, and `name` overrides the label. A place whose folder is gone keeps its row in the file,
//! and nodes this release does not know are copied back untouched.

mod builtin;

use crate::config;
use crate::icons;
use builtin::{BUILTINS, builtin};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use rune::Icon;
use rune_kit::glyphs;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const FILE: &str = "places.kdl";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Favourites,
    Locations,
}

impl Section {
    pub const ALL: [Section; 2] = [Section::Favourites, Section::Locations];

    pub fn title(self) -> &'static str {
        match self {
            Section::Favourites => "Favourites",
            Section::Locations => "Locations",
        }
    }

    pub fn key(self) -> &'static str {
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

#[derive(Clone)]
pub struct Place {
    pub id: String,
    pub name: String,
    pub icon: &'static Icon,
    pub path: PathBuf,
    pub hidden: bool,
}

impl Place {
    pub fn is_available(&self) -> bool {
        !self.hidden && self.path.exists()
    }

    pub fn folder(&self) -> PathBuf {
        if self.path.is_dir() {
            return self.path.clone();
        }
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.path.clone())
    }
}

fn label_for(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

fn icon_for(id: &str) -> &'static Icon {
    if id.starts_with("file:") {
        &glyphs::FILE
    } else if id.starts_with("volume:") {
        &icons::DRIVE
    } else {
        &glyphs::FOLDER
    }
}

#[derive(Default)]
pub struct Places {
    favourites: Vec<Place>,
    locations: Vec<Place>,
    path: Option<PathBuf>,
    extra: Vec<KdlNode>,
    pub error: Option<String>,
}

impl Places {
    pub fn load() -> Places {
        let Some(dir) = config::state_dir() else {
            return Places::default();
        };
        Places::at(dir.join(FILE))
    }

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

    pub fn places(&self, section: Section) -> &[Place] {
        section.list(self)
    }

    pub fn get(&self, id: &str) -> Option<&Place> {
        Section::ALL
            .iter()
            .flat_map(|section| section.list(self))
            .find(|place| place.id == id)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        for section in Section::ALL {
            if let Some(place) = section
                .list_mut(self)
                .iter_mut()
                .find(|place| place.id == id)
            {
                if place.hidden {
                    return false;
                }
                place.hidden = true;
                return true;
            }
        }
        false
    }

    pub fn rename(&mut self, id: &str, name: &str) -> bool {
        for section in Section::ALL {
            if let Some(place) = section
                .list_mut(self)
                .iter_mut()
                .find(|place| place.id == id)
            {
                if place.name == name {
                    return false;
                }
                place.name = name.to_owned();
                return true;
            }
        }
        false
    }

    pub fn move_path(&mut self, old: &Path, new: &Path) -> bool {
        let derived = label_for(old);
        let mut moved = false;
        for section in Section::ALL {
            for place in section.list_mut(self) {
                if place.path != old {
                    continue;
                }
                moved = true;
                if place.name == derived {
                    place.name = label_for(new);
                }
                place.path = new.to_path_buf();
                if let Some((kind @ ("folder" | "file"), _)) = place.id.split_once(':') {
                    place.id = format!("{kind}:{}", new.display());
                }
            }
        }
        moved
    }

    pub fn insert(&mut self, section: Section, index: usize, paths: &[PathBuf]) -> bool {
        let mut fresh = Vec::new();
        for path in paths {
            if !path.exists() || fresh.iter().any(|place: &Place| place.path == *path) {
                continue;
            }
            match self.find_path(path).cloned() {
                Some(place) if !place.hidden => continue,
                Some(mut place) => {
                    self.take(&place.id);
                    place.hidden = false;
                    fresh.push(place);
                }
                None => {
                    let id = place_id(path);
                    fresh.push(Place {
                        name: label_for(path),
                        icon: icon_for(&id),
                        id,
                        path: path.clone(),
                        hidden: false,
                    });
                }
            }
        }
        if fresh.is_empty() {
            return false;
        }
        let at = {
            let list = section.list(self);
            list.iter()
                .enumerate()
                .filter(|(_, place)| place.is_available())
                .map(|(at, _)| at)
                .nth(index)
                .unwrap_or(list.len())
        };
        let list = section.list_mut(self);
        for (offset, place) in fresh.into_iter().enumerate() {
            list.insert(at + offset, place);
        }
        true
    }

    fn find_path(&self, path: &Path) -> Option<&Place> {
        Section::ALL
            .iter()
            .flat_map(|section| section.list(self))
            .find(|place| place.path == path)
    }

    fn take(&mut self, id: &str) -> Option<Place> {
        for section in Section::ALL {
            let list = section.list_mut(self);
            if let Some(at) = list.iter().position(|place| place.id == id) {
                return Some(list.remove(at));
            }
        }
        None
    }

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

    pub fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        match config::write_atomic(&path, &render(self)) {
            Ok(()) => self.error = None,
            Err(err) => self.error = Some(err),
        }
    }

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

struct Stored {
    id: String,
    path: Option<PathBuf>,
    name: Option<String>,
    hidden: bool,
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
                hidden: self.hidden,
            });
        }
        let path = self.path?;
        Some(Place {
            name: self.name.unwrap_or_else(|| label_for(&path)),
            icon: icon_for(&self.id),
            id: self.id,
            path,
            hidden: self.hidden,
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
            _ => parsed.extra.push(node.clone()),
        }
    }
    Ok(parsed)
}

fn stored(node: &KdlNode) -> Vec<Stored> {
    node.children()
        .into_iter()
        .flat_map(|kids| kids.nodes())
        .filter(|child| child.name().value() == "place")
        .filter_map(|child| {
            let id = child.get(0).and_then(|entry| entry.as_string())?;
            let mut path = None;
            let mut name = None;
            let mut hidden = false;
            for entry in child.entries() {
                let Some(key) = entry.name().map(|name| name.value()) else {
                    continue;
                };
                let value = entry.value().as_string().map(str::to_owned);
                match key {
                    "path" => path = value.map(PathBuf::from),
                    "name" => name = value,
                    "hidden" => hidden = entry.value().as_bool().unwrap_or(false),
                    _ => {}
                }
            }
            Some(Stored {
                id: id.to_owned(),
                path,
                name,
                hidden,
            })
        })
        .collect()
}

fn render(places: &Places) -> String {
    let mut doc = KdlDocument::new();
    for node in &places.extra {
        doc.nodes_mut().push(node.clone());
    }
    for section in Section::ALL {
        let mut node = KdlNode::new(section.key());
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

fn place_id(path: &Path) -> String {
    format!(
        "{}:{}",
        if path.is_dir() { "folder" } else { "file" },
        path.display()
    )
}

fn place_node(place: &Place) -> KdlNode {
    let mut node = KdlNode::new("place");
    node.push(KdlEntry::new(place.id.clone()));
    let builtin = builtin(&place.id);
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
    if place.hidden {
        node.push(KdlEntry::new_prop("hidden", true));
    }
    node
}
