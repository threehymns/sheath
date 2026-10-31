//! The places every launch knows about: the home folder and its usual folders, the file
//! system's root, and the temporary folder. The state file names the ones the user keeps and
//! where they moved them; anything it leaves out is added at the end of its section.

use super::{Place, Section};
use crate::app::{home_dir, root_dir};
use crate::icons;
use rune::Icon;
use rune_kit::glyphs;
use std::path::PathBuf;

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

pub(super) struct Builtin {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) icon: &'static Icon,
    pub(super) section: Section,
    anchor: Anchor,
}

/// The places every launch knows about, in their starting order. The file names the ones the
/// user keeps and where they moved them; anything it leaves out is added at the end of its
/// section, so a place a later release adds still turns up.
pub(super) const BUILTINS: [Builtin; 7] = [
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
    pub(super) fn path(&self) -> Option<PathBuf> {
        match self.anchor {
            Anchor::Home => home_dir(),
            Anchor::UnderHome(folder) => home_dir().map(|home| home.join(folder)),
            Anchor::Root => Some(root_dir()),
            Anchor::Temp => Some(std::env::temp_dir()),
        }
    }

    /// Whether the folder is there to show. The temporary folder is hidden when it is the root
    /// the root place already opens.
    pub(super) fn exists(&self) -> bool {
        self.path().is_some_and(|path| path.is_dir())
            && !(self.id == "temp" && self.path() == Some(root_dir()))
    }

    pub(super) fn place(&self) -> Option<Place> {
        Some(Place {
            id: self.id.to_owned(),
            name: self.name.to_owned(),
            icon: self.icon,
            path: self.path()?,
            hidden: false,
        })
    }
}

pub(super) fn builtin(id: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|builtin| builtin.id == id)
}
