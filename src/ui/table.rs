//! The file table: its header, the columns, and the virtualised rows the app draws itself.

use crate::app::{App, ROW_HEIGHT};
use crate::format;
use crate::icons;
use rune::{
    Align, Cursor, Drag, El, GrowthSpec, Phase, ScrollSpec, Spring, Transition, Tx, col, el, icon,
    row, text,
};
use rune_kit::{Kit, SortDir, TableStyle, file_icon, glyphs};
use std::borrow::Cow;

/// The column header's height, which the grid's sort toolbar shares so the two layouts start
/// their items at the same place.
pub(super) const HEADER_HEIGHT: f32 = 32.0;

/// The table's measurements, shared with anything else that draws its parts.
pub(super) fn browser_style(k: &Kit) -> TableStyle {
    TableStyle {
        row_height: ROW_HEIGHT,
        header_height: HEADER_HEIGHT,
        ..TableStyle::from_kit(k)
    }
}

pub(super) fn file_table(s: &App, k: &'static Kit) -> El {
    let style = browser_style(k);
    // The viewport's own offset, told in the frame that scrolls it, so the wheel and its coast
    // start where the rows really are rather than where their rects last reported.
    let told = s.scroll_told.clone();
    // The presses, the keys and the drags live on the scrolling viewport, so a press on the
    // empty space below the rows starts a selection box too.
    let viewport = col()
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
        // The bounce moves the rows without moving the viewport, so its edges stay put.
        .child(file_rows(s, k, style).offset(0.0, s.overshoot()));
    col()
        .key("file-table")
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(file_header(s, k, &style))
        .child(viewport)
}

/// The column titles: a click sorts by that column, another click reverses it, and the edge of
/// a fixed column drags it wider.
fn file_header(s: &App, k: &Kit, style: &TableStyle) -> El {
    let (column, direction) = s.sort_column();
    row()
        .key("header")
        .role("row")
        .w_full()
        .h(style.header_height)
        .center_y()
        .child(
            el().key("header-rule")
                .pin(Some(0.0), None, Some(0.0), Some(0.0))
                .h(1.0)
                .fill(style.rule),
        )
        .child(header_cell(
            k,
            style,
            "name",
            "Name",
            None,
            Align::Start,
            column,
            direction,
        ))
        .child(header_cell(
            k,
            style,
            "size",
            "Size",
            Some(s.size_width),
            Align::End,
            column,
            direction,
        ))
        .child(header_cell(
            k,
            style,
            "modified",
            "Date Modified",
            Some(s.date_width),
            Align::Start,
            column,
            direction,
        ))
}

#[allow(clippy::too_many_arguments)]
fn header_cell(
    k: &Kit,
    style: &TableStyle,
    id: &'static str,
    title: &'static str,
    width: Option<f32>,
    align: Align,
    column: &str,
    direction: SortDir,
) -> El {
    let mut cell = row()
        .role("columnheader")
        .h_full()
        .px(style.cell_pad)
        .center_y()
        .cursor(Cursor::Pointer)
        .hover(k.c.hover)
        .on_press(move |s: &mut App| s.sort_by(id))
        .child(sort_label(k, style, id, title, column, direction));
    cell = match align {
        Align::End => cell.justify_end(),
        Align::Center => cell.justify_center(),
        Align::Start => cell,
    };
    match width {
        Some(width) => cell
            .w(width)
            .child(column_resize(id, title, width, style.rule))
            .key(id),
        // The name column takes what the fixed ones leave.
        None => cell.grow(1.0).shrink().min_w(200.0).key(id),
    }
}

/// A column's title and, when it is the sorted column, the arrow that points the way it sorts.
/// The header and the grid's sort toolbar both draw their controls from this.
pub(super) fn sort_label(
    k: &Kit,
    style: &TableStyle,
    id: &'static str,
    title: &'static str,
    column: &str,
    direction: SortDir,
) -> El {
    let sorted = (column == id).then_some(direction);
    let label = text(title)
        .font(style.header_font)
        .color(if sorted.is_some() {
            style.text
        } else {
            style.header_text
        })
        .truncate()
        .shrink()
        .min_w(0.0);
    let mut inner = row().gap(2.0).center_y().min_w(0.0).shrink().child(label);
    if let Some(direction) = sorted {
        // One chevron, turned over for the ascending sort, so reversing a column springs the
        // arrow round rather than swapping one glyph for another.
        let turn = if direction == SortDir::Ascending {
            std::f32::consts::PI
        } else {
            0.0
        };
        inner = inner.child(
            icon(k.glyph.chevron_down, 14.0)
                .key("sort")
                .color(style.text)
                .rotate(turn),
        );
    }
    inner
}

/// The drag target on a header cell's trailing edge: dragging it resizes the column, a double
/// click puts it back.
fn column_resize(id: &'static str, title: &'static str, width: f32, rule: rune::Color) -> El {
    row()
        .key("resize")
        .label(format!("Resize {title}"))
        .pin(None, Some(6.0), Some(-4.0), Some(6.0))
        .w(9.0)
        .justify_center()
        .cursor(Cursor::ResizeColumn)
        .on_drag(
            move |s: &mut App, drag: &mut Drag, _: &mut Tx| match drag.phase {
                Phase::Down => drag.data = width,
                Phase::Move | Phase::Up => {
                    if !drag.cancelled() {
                        s.resize_column(id, drag.data + drag.dx);
                    }
                }
            },
        )
        .on_double_press(move |s: &mut App| s.reset_column(id))
        .child(el().key("line").w(1.0).h_full().fill(rule))
}

/// The rows themselves: app-drawn so the app owns the selection, the clicks and the box a drag
/// draws over them.
fn file_rows(s: &App, k: &'static Kit, style: TableStyle) -> El {
    col()
        .key("file-rows")
        .w_full()
        // Rows land where they belong: no enter or exit fade, no hover or selection
        // crossfade, and no glide when a sort, a filter or a refresh moves them. The list's
        // own motion is the viewport's scroll spring and the app's overshoot, outside this
        // subtree.
        .motion(Spring::NONE)
        .instant_changes()
        .hover_motion_within(Spring::NONE, Spring::NONE)
        .growth(GrowthSpec {
            spring: Spring::NONE,
            ..GrowthSpec::default()
        })
        .virtual_rows(
            s.visible.len(),
            ROW_HEIGHT,
            |s: &App, i| Cow::Borrowed(s.entries[s.visible[i]].id.as_str()),
            move |s: &App, i| file_row(s, i, k, style),
        )
}

fn file_row(s: &App, i: usize, k: &'static Kit, style: TableStyle) -> El {
    let Some(&index) = s.visible.get(i) else {
        return row().key(format!("row-{i}"));
    };
    let entry = &s.entries[index];
    let mut line = row()
        .key(entry.id.clone())
        .role("row")
        .w_full()
        .h(ROW_HEIGHT)
        .center_y()
        .transition(Transition::none())
        // The row's own hover overlay lands with the pointer, rather than fading: the
        // nearest pair wins over anything the shell set above the list.
        .hover_motion(Spring::NONE, Spring::NONE);
    if s.is_cut(&entry.path) {
        // Cut rows wait, dimmed, for a paste.
        line = line.opacity(0.45);
    }
    line = if s.drop_row() == Some(i) {
        // The folder a dragged selection would land in.
        line.fill(k.alpha(k.c.text_accent, 0.3))
            .outline(1.0, -1.0, k.c.border_focus)
    } else if s.is_selected(&entry.id) {
        line.fill(style.selected)
    } else if i % 2 == 1 {
        line.fill(style.stripe)
            .hover(style.stripe.mix(style.selected, 0.5))
    } else {
        line.hover(style.stripe.mix(style.selected, 0.5))
    };
    line.child(name_cell(s, i, k, style))
        .child(size_cell(s, i, style, s.size_width))
        .child(modified_cell(s, i, style, s.date_width))
}

fn name_cell(s: &App, i: usize, k: &'static Kit, style: TableStyle) -> El {
    let Some(&index) = s.visible.get(i) else {
        return row();
    };
    let entry = &s.entries[index];
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
    let mut content = row()
        .gap(8.0)
        .center_y()
        .w_full()
        .h_full()
        .child(icon(glyph, 16.0).color(icon_color))
        .child(
            text(entry.name.clone())
                .font(style.font)
                .color(if entry.is_symlink {
                    style.muted
                } else {
                    style.text
                })
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .truncate(),
        );
    if entry.is_symlink {
        content = content.child(icon(&icons::LINK, 11.0).color(k.c.icon_faint));
    }
    row()
        .w_full()
        .h_full()
        .grow(1.0)
        .shrink()
        .min_w(200.0)
        .px(style.cell_pad)
        .center_y()
        .child(content)
}

fn size_cell(s: &App, i: usize, style: TableStyle, width: f32) -> El {
    let Some(&index) = s.visible.get(i) else {
        return row();
    };
    let entry = &s.entries[index];
    let (body, font) = if entry.is_dir {
        ("—".to_owned(), style.font)
    } else {
        (format::size(entry.size), style.mono)
    };
    row()
        .w(width)
        .h_full()
        .px(style.cell_pad)
        .justify_end()
        .center_y()
        .child(text(body).font(font).color(style.muted).truncate().shrink())
}

fn modified_cell(s: &App, i: usize, style: TableStyle, width: f32) -> El {
    let Some(&index) = s.visible.get(i) else {
        return row();
    };
    row().w(width).h_full().px(style.cell_pad).center_y().child(
        text(format::table_time(s.entries[index].modified))
            .font(style.font)
            .color(style.muted)
            .truncate()
            .shrink(),
    )
}

pub(super) fn empty_state(s: &App, k: &'static Kit) -> El {
    let searching = !s.search.text.trim().is_empty();
    let (glyph, title, subtitle) = if s.error.is_some() {
        (
            &glyphs::FOLDER,
            "Can’t open this folder",
            "Pick another place in the sidebar.",
        )
    } else if searching {
        (
            k.glyph.search,
            "No matches",
            "Try a different search or show hidden files.",
        )
    } else {
        (
            &glyphs::FOLDER_OPEN,
            "This folder is empty",
            "Use New Folder to put something here.",
        )
    };
    col()
        .key("empty")
        .w_full()
        .h_full()
        .center()
        .gap(6.0)
        .child(icon(glyph, 40.0).color(k.c.icon_faint))
        .child(text(title).font(k.ty.ui_medium).color(k.c.text_muted))
        .child(
            text(subtitle)
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .align(Align::Center),
        )
}
