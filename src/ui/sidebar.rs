//! The sidebar: the places the app can jump to, dragged into the order the user wants, and
//! the drop targets a dragged row can land on.

use crate::app::App;
use crate::places::{Place, Section};
use rune::El;
use rune_kit::shell::Reorder;
use rune_kit::{Kit, Sidebar, SidebarRow, SidebarSection};

pub(super) fn sidebar(s: &App, k: &'static Kit) -> El {
    Sidebar::new()
        .inset(10.0)
        .row_gap(1.0)
        .section(place_section(s, k, Section::Favourites))
        .section(place_section(s, k, Section::Locations))
        .build(k)
}

/// One section's rows, numbered among themselves: a place is dragged within its own section,
/// so a row never lands among the other one's.
fn place_section(s: &App, k: &'static Kit, section: Section) -> SidebarSection {
    let shown: Vec<&Place> = s
        .places
        .places(section)
        .iter()
        .filter(|place| place.is_available())
        .collect();
    let count = shown.len();
    let mut rows = SidebarSection::new(section.title());
    for (index, place) in shown.into_iter().enumerate() {
        rows = rows.row(place_row(s, k, section, index, count, place));
    }
    rows
}

/// One place in the sidebar: the folder it opens, the order it sits in, and the rect a dragged
/// row can be dropped on. A press that moves reorders the place; a press that doesn't opens
/// the folder.
fn place_row(
    s: &App,
    k: &'static Kit,
    section: Section,
    index: usize,
    count: usize,
    place: &Place,
) -> El {
    let path = place.path.clone();
    let target = place.path.clone();
    let id = place.id.clone();
    let highlight = s.drop_place() == Some(place.id.as_str());
    let (drag, access) = match section {
        Section::Favourites => (
            &s.favourites_drag,
            favourites_drag as fn(&mut App) -> &mut Reorder,
        ),
        Section::Locations => (
            &s.locations_drag,
            locations_drag as fn(&mut App) -> &mut Reorder,
        ),
    };
    SidebarRow::new(place.name.clone())
        .icon(place.icon)
        .selected(s.cwd == place.path || highlight)
        .on_select(move |s: &mut App, _| {
            // The release that ends a reorder arrives as a press too, and it must not open the
            // folder the row was dragged over.
            if !s.take_place_drag() {
                s.navigate(path.clone());
            }
        })
        .reorder(drag, index, count, access, move |s: &mut App, from, to| {
            s.place_dragged = true;
            s.move_place(section, from, to);
        })
        .build(k)
        .key(format!("place-{id}"))
        .on_layout(move |s: &mut App, rect| {
            s.places_at.insert(id.clone(), (rect, target.clone()));
        })
}

fn favourites_drag(s: &mut App) -> &mut Reorder {
    &mut s.favourites_drag
}

fn locations_drag(s: &mut App) -> &mut Reorder {
    &mut s.locations_drag
}
