//! The grid view: a virtualised wall of tiles, one per item, drawn by the app so a press, a
//! box and a drop read the same state the table's rows do.

use crate::app::{App, GRID_PAD, GRID_ROW_HEIGHT, TILE_GAP, TILE_HEIGHT, TILE_WIDTH};
use rune::{Align, El, GrowthSpec, ScrollSpec, Tx, col, icon, row, text};
use rune_kit::{Kit, file_icon, glyphs};
use std::borrow::Cow;

pub(super) fn file_grid(s: &App, k: &'static Kit) -> El {
    let told = s.scroll_told.clone();
    let columns = s.grid_columns();
    let rows = s.grid_rows();
    // The press, the keys, the drags and the wheel live on the scrolling viewport, as they do
    // in the table, so a press on the space past the last tile starts a box too.
    col().key("file-grid").w_full().h_full().min_h(0.0).child(
        col()
            .key("files-table")
            .role("grid")
            .label("Files")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .focusable(true)
            .on_key(|s: &mut App, event, tx| crate::app::list_key(s, event, tx))
            .on_wheel(|s: &mut App, wheel, tx| s.list_wheel(wheel, tx))
            .on_drag(|s: &mut App, drag, tx| crate::app::list_drag(s, drag, tx))
            .on_secondary_press(|s: &mut App, press, _: &mut Tx| s.row_menu_at(press))
            .on_layout(|s: &mut App, rect| s.rows_view = rect)
            .on_scroll_offset(move |at| told.set(at))
            // The bounce moves the tiles without moving the viewport, so its edges stay put.
            .child(
                col()
                    .key("file-grid-rows")
                    .w_full()
                    .growth(GrowthSpec::default())
                    .virtual_rows(
                        rows,
                        GRID_ROW_HEIGHT,
                        move |s: &App, r| match s.visible.get(r * columns) {
                            Some(&i) => Cow::Owned(format!("grid-row:{}", s.entries[i].id)),
                            None => Cow::Borrowed("grid-row"),
                        },
                        move |s: &App, r| grid_row(s, r, columns, k),
                    )
                    .offset(0.0, s.overshoot()),
            ),
    )
}

/// One grid row: as many tiles as the viewport fits across, and none past the last item.
fn grid_row(s: &App, row_index: usize, columns: usize, k: &'static Kit) -> El {
    let mut line = row()
        .key(format!("grid-row-{row_index}"))
        .w_full()
        .h(GRID_ROW_HEIGHT)
        .px(GRID_PAD)
        .gap(TILE_GAP);
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
        // Cut tiles wait, dimmed, for a paste.
        tile = tile.opacity(0.45);
    }
    if s.drop_row() == Some(index) {
        // The folder a dragged selection would land in.
        tile.fill(k.alpha(k.c.text_accent, 0.3))
            .outline(1.0, -1.0, k.c.border_focus)
    } else if s.is_selected(&entry.id) {
        tile.fill(k.alpha(k.c.text_accent, 0.16))
    } else {
        tile.hover(k.c.hover)
    }
}
