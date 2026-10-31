//! The sidebar: the places the app can jump to, dragged into the order the user wants, and
//! the drop targets a dragged row can land on.

use crate::app::{App, PlaceAt};
use crate::places::{Place, Section};
use rune::{El, Tx};
use rune_kit::shell::Reorder;
use rune_kit::{Kit, Sidebar, SidebarRow, SidebarSection};

pub(super) fn sidebar(s: &App, k: &'static Kit) -> El {
    Sidebar::new()
        .inset(10.0)
        .row_gap(1.0)
        .child(place_section(s, k, Section::Favourites))
        .child(place_section(s, k, Section::Locations))
        .build(k)
}

/// One section: its heading and rows, numbered among themselves so a place is dragged within
/// its own section, and its rect, which a row dropped there is added to.
fn place_section(s: &App, k: &'static Kit, section: Section) -> El {
    let shown: Vec<&Place> = s
        .places
        .places(section)
        .iter()
        .filter(|place| place.is_available())
        .collect();
    let count = shown.len();
    // A file drag between the rows is marked by a line drawn over them: the rows keep their
    // places, so aiming at an insertion shifts nothing.
    let mut rows = SidebarSection::new(section.title());
    for (index, place) in shown.into_iter().enumerate() {
        rows = rows.row(place_row(s, k, section, index, count, place));
    }
    rows.build(k)
        .key(format!("places-{}", section.key()))
        .on_layout(move |s: &mut App, rect| {
            s.sections_at.insert(section, rect);
        })
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
    let menu_id = place.id.clone();
    let highlight = s.drop_place() == Some(place.id.as_str());
    let opens = place.folder();
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
        .selected(s.cwd == opens || highlight)
        .on_select(move |s: &mut App, _| {
            // The release that ends a reorder arrives as a press too, and it must not open the
            // folder the row was dragged over.
            if !s.take_place_drag() {
                s.open_place(path.clone());
            }
        })
        .reorder(drag, index, count, access, move |s: &mut App, from, to| {
            s.place_dragged = true;
            s.move_place(section, from, to);
        })
        .build(k)
        .key(format!("place-{id}"))
        .on_secondary_press(move |s: &mut App, press, _: &mut Tx| {
            s.open_place_menu(menu_id.clone(), press);
        })
        .on_layout(move |s: &mut App, rect| {
            s.places_at.insert(
                id.clone(),
                PlaceAt {
                    rect,
                    path: target.clone(),
                    section,
                    index,
                },
            );
        })
}

fn favourites_drag(s: &mut App) -> &mut Reorder {
    &mut s.favourites_drag
}

fn locations_drag(s: &mut App) -> &mut Reorder {
    &mut s.locations_drag
}
