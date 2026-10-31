use crate::app::{
    App, FILES, GRID_PAD, GRID_ROW_HEIGHT, GRID_TOP_MARGIN, TILE_GAP, TILE_HEIGHT, TILE_WIDTH,
};
use rune::{Align, Cursor, El, GrowthSpec, Spring, Transition, Tx, col, el, icon, row, text};
use rune_kit::{Kit, SortDir, file_icon, glyphs};
use std::borrow::Cow;

use super::table::{HEADER_HEIGHT, browser_style, mount, sort_label};

const SORT_BUTTON_HEIGHT: f32 = 20.0;

pub(super) fn file_grid(s: &App, k: &'static Kit, cut: bool) -> El {
    let columns = s.grid_columns();
    let rows = s.grid_rows();
    col()
        .key("file-grid")
        .w_full()
        .h_full()
        .min_h(0.0)
        .transition(Transition::none())
        .child(grid_toolbar(s, k).transition(mount(cut)))
        .child(super::scroll_area::scrolled(
            s,
            FILES,
            col()
                .role("grid")
                .label("Files")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .focusable(true)
                .no_focus_ring()
                .on_key(|s: &mut App, event, tx| crate::app::list_key(s, event, tx))
                .on_drag(|s: &mut App, drag, tx| crate::app::list_drag(s, drag, tx))
                .on_secondary_press(|s: &mut App, press, _: &mut Tx| s.row_menu_at(press))
                .transition(mount(cut)),
            col()
                .key("file-grid-rows")
                .w_full()
                .pt(GRID_TOP_MARGIN)
                .transition(mount(cut))
                .motion(Spring::NONE)
                .instant_changes()
                .hover_motion_within(Spring::NONE, Spring::NONE)
                .growth(GrowthSpec {
                    spring: Spring::NONE,
                    ..GrowthSpec::default()
                })
                .virtual_rows(
                    rows,
                    GRID_ROW_HEIGHT,
                    move |s: &App, r| match s.visible.get(r * columns) {
                        Some(&i) => Cow::Owned(format!("grid-row:{}", s.entries[i].id)),
                        None => Cow::Borrowed("grid-row"),
                    },
                    move |s: &App, r| grid_row(s, r, columns, k),
                )
                .virtual_rows_version((s.keys_version(), columns)),
        ))
}

fn grid_toolbar(s: &App, k: &'static Kit) -> El {
    let style = browser_style(k);
    let (column, direction) = s.sort_column();
    let sort = |id, title| grid_sort(k, &style, id, title, column, direction);
    row()
        .key("grid-toolbar")
        .role("row")
        .label("Sort")
        .w_full()
        .h(HEADER_HEIGHT)
        .px(GRID_PAD)
        .gap(2.0)
        .center_y()
        .child(
            el().key("grid-toolbar-rule")
                .pin(Some(0.0), None, Some(0.0), Some(0.0))
                .h(1.0)
                .fill(style.rule),
        )
        .child(sort("name", "Name"))
        .child(sort("size", "Size"))
        .child(sort("modified", "Date Modified"))
}

fn grid_sort(
    k: &Kit,
    style: &rune_kit::TableStyle,
    id: &'static str,
    title: &'static str,
    column: &str,
    direction: SortDir,
) -> El {
    row()
        .key(format!("grid-sort-{id}"))
        .role("button")
        .label(format!("Sort by {title}"))
        .h(SORT_BUTTON_HEIGHT)
        .px(10.0)
        .center_y()
        .radius(k.radius.sm)
        .cursor(Cursor::Pointer)
        .hover(k.c.hover)
        .on_press(move |s: &mut App| s.sort_by(id))
        .child(sort_label(k, style, id, title, column, direction))
}

fn grid_row(s: &App, row_index: usize, columns: usize, k: &'static Kit) -> El {
    let mut line = row()
        .key(format!("grid-row-{row_index}"))
        .w_full()
        .h(GRID_ROW_HEIGHT)
        .px(GRID_PAD)
        .gap(TILE_GAP)
        .transition(Transition::none());
    for column in 0..columns {
        let index = row_index * columns + column;
        if index >= s.visible.len() {
            break;
        }
        line = line.child(grid_tile(s, index, k));
    }
    line
}

fn grid_tile(s: &App, index: usize, k: &'static Kit) -> El {
    let Some(&entry_index) = s.visible.get(index) else {
        return row().key(format!("grid-tile-{index}"));
    };
    let entry = &s.entries[entry_index];
    let glyph = if entry.is_dir {
        &glyphs::FOLDER
    } else {
        file_icon(&entry.name)
    };
    let icon_color = if entry.is_symlink {
        k.c.icon_faint
    } else if entry.is_dir {
        k.c.icon_accent
    } else {
        k.c.icon_muted
    };
    let mut tile = col()
        .key(entry.id.clone())
        .role("gridcell")
        .label(entry.name.clone())
        .w(TILE_WIDTH)
        .h(TILE_HEIGHT)
        .px(6.0)
        .py(8.0)
        .gap(6.0)
        .radius(k.radius.sm)
        .items_center()
        .justify_center()
        .transition(Transition::none())
        .hover_motion(Spring::NONE, Spring::NONE)
        .child(icon(glyph, 34.0).color(icon_color))
        .child(
            text(entry.name.clone())
                .font(k.ty.caption())
                .color(if entry.is_symlink {
                    k.c.text_muted
                } else {
                    k.c.text_base
                })
                .align(Align::Center)
                .w_full()
                .wrap()
                .max_lines(2),
        );
    if s.is_cut(&entry.path) {
        tile = tile.opacity(0.45);
    }
    if s.drop_row() == Some(index) {
        tile.fill(k.alpha(k.c.text_accent, 0.3))
            .outline(1.0, -1.0, k.c.border_focus)
    } else if s.is_selected(&entry.id) {
        tile.fill(k.alpha(k.c.text_accent, 0.16))
    } else {
        tile.hover(k.c.hover)
    }
}
