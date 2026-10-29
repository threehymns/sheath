//! The sidebar: the places the app can jump to, and the drop targets a dragged row can
//! land on.

use crate::app::App;
use crate::icons;
use rune::{El, Icon};
use rune_kit::{Kit, Sidebar, SidebarRow, SidebarSection, glyphs};
use std::path::PathBuf;

use super::*;

struct Place {
    id: &'static str,
    name: &'static str,
    icon: &'static Icon,
    path: PathBuf,
}

fn places() -> (Vec<Place>, Vec<Place>) {
    let mut favourites = Vec::new();
    if let Some(home) = home_dir() {
        favourites.push(Place {
            id: "home",
            name: "Home",
            icon: &icons::HOUSE,
            path: home.clone(),
        });
        for (id, name, icon, folder) in [
            ("desktop", "Desktop", &icons::DESKTOP, "Desktop"),
            ("documents", "Documents", &icons::DOCUMENTS, "Documents"),
            ("downloads", "Downloads", &icons::DOWNLOADS, "Downloads"),
            ("pictures", "Pictures", &icons::PICTURES, "Pictures"),
        ] {
            let path = home.join(folder);
            if path.is_dir() {
                favourites.push(Place {
                    id,
                    name,
                    icon,
                    path,
                });
            }
        }
    }
    let root = root_dir();
    let mut locations = vec![Place {
        id: "root",
        name: "Computer",
        icon: &icons::DRIVE,
        path: root.clone(),
    }];
    let temp = std::env::temp_dir();
    if temp != root && temp.is_dir() {
        locations.push(Place {
            id: "temp",
            name: "Temporary",
            icon: &glyphs::FOLDER,
            path: temp,
        });
    }
    (favourites, locations)
}

pub(super) fn sidebar(s: &App, k: &'static Kit) -> El {
    let (favourites, locations) = places();
    let mut fav = SidebarSection::new("Favourites");
    for place in favourites {
        fav = fav.row(place_row(s, &place, k));
    }
    let mut loc = SidebarSection::new("Locations");
    for place in locations {
        loc = loc.row(place_row(s, &place, k));
    }
    Sidebar::new()
        .inset(10.0)
        .row_gap(1.0)
        .section(fav)
        .section(loc)
        .build(k)
}

/// One place in the sidebar: the folder it opens, and the rect a dragged row can be dropped on.
fn place_row(s: &App, place: &Place, k: &'static Kit) -> El {
    let path = place.path.clone();
    let target = place.path.clone();
    let id = place.id;
    let highlight = s.drop_place() == Some(id);
    SidebarRow::new(place.name)
        .icon(place.icon)
        .selected(s.cwd == place.path || highlight)
        .on_select(move |s: &mut App, _| s.navigate(path.clone()))
        .build(k)
        .key(format!("place-{id}"))
        .on_layout(move |s: &mut App, rect| {
            s.places_at.insert(id, (rect, target.clone()));
        })
}
