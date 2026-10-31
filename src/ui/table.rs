use crate::app::{App, FILES, ROW_HEIGHT};
use crate::format;
use crate::icons;
use chrono::{DateTime, Local};
use rune::{
    Align, Cursor, Drag, El, GrowthSpec, Phase, Spring, Transition, Tx, col, el, icon, row, text,
};
use rune_kit::{Kit, SortDir, TableStyle, file_icon, glyphs};
use std::borrow::Cow;

// Header height, shared by the grid's sort toolbar so both layouts start items at the same y.
pub(super) const HEADER_HEIGHT: f32 = 32.0;

pub(super) fn browser_style(k: &Kit) -> TableStyle {
    TableStyle {
        row_height: ROW_HEIGHT,
        header_height: HEADER_HEIGHT,
        ..TableStyle::from_kit(k)
    }
}

// Rune fades every element on mount, so each part cuts in; the root's own Transition::none is
// not enough.
pub(super) fn mount(cut: bool) -> Transition {
    if cut {
        Transition::none()
    } else {
        Transition::opacity()
    }
}

pub(super) fn file_table(s: &App, k: &'static Kit, cut: bool) -> El {
    let style = browser_style(k);
    let viewport = col()
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
        .transition(mount(cut));
    col()
        .key("file-table")
        .w_full()
        .h_full()
        .min_h(0.0)
        .transition(Transition::none())
        .child(file_header(s, k, &style).transition(mount(cut)))
        .child(super::scroll_area::scrolled(
            s,
            FILES,
            viewport,
            file_rows(s, k, style).transition(mount(cut)),
        ))
}

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
        None => cell.grow(1.0).shrink().min_w(200.0).key(id),
    }
}

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

fn file_rows(s: &App, k: &'static Kit, style: TableStyle) -> El {
    // One clock reading serves every row; the labels only need the day and the hour.
    let now = Local::now();
    col()
        .key("file-rows")
        .w_full()
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
            move |s: &App, i| file_row(s, i, k, style, now),
        )
        .virtual_rows_version(s.keys_version())
}

fn file_row(s: &App, i: usize, k: &'static Kit, style: TableStyle, now: DateTime<Local>) -> El {
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
        .hover_motion(Spring::NONE, Spring::NONE);
    if s.is_cut(&entry.path) {
        line = line.opacity(0.45);
    }
    line = if s.drop_row() == Some(i) {
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
        .child(modified_cell(s, i, style, s.date_width, now))
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
    let id = entry.id.clone();
    let name = text(entry.name.clone())
        .key(entry.name_key.clone())
        .font(style.font)
        .color(if entry.is_symlink {
            style.muted
        } else {
            style.text
        })
        .shrink()
        .min_w(0.0)
        .truncate()
        .on_layout(move |s: &mut App, rect| {
            s.name_at.insert(id.clone(), rect[0] + rect[2]);
        });
    let mut content = row()
        .gap(8.0)
        .center_y()
        .w_full()
        .h_full()
        .child(icon(glyph, 16.0).color(icon_color))
        .child(name);
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

fn modified_cell(s: &App, i: usize, style: TableStyle, width: f32, now: DateTime<Local>) -> El {
    let Some(&index) = s.visible.get(i) else {
        return row();
    };
    row().w(width).h_full().px(style.cell_pad).center_y().child(
        text(format::table_time(s.entries[index].modified, now))
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
        .transition(Transition::none())
        .child(icon(glyph, 40.0).color(k.c.icon_faint))
        .child(text(title).font(k.ty.ui_medium).color(k.c.text_muted))
        .child(
            text(subtitle)
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .align(Align::Center),
        )
}
