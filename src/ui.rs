//! The Rune view. All layout, painting and input come from Rune; this file only assembles the
//! app's state into elements.

use std::borrow::Cow;
use std::path::{Component, PathBuf};

use rune::{
    Align, Cursor, Cx, Drag, El, GrowthSpec, Icon, Key, Phase, PointerPress, ScrollSpec, Tx, col,
    el, icon, row, text,
};
use rune_kit::{
    Button, ButtonVariant, ControlSize, Dialog, IconButton, IconButtonVariant, Kit, MenuItem,
    ShortcutRecorder, Sidebar, SidebarRow, SidebarSection, SortDir, SplitView, Switch, TableStyle,
    TextInput, Toolbar, context_menu, file_icon, glyphs, scrim,
};
use rune_theme_opencode as theme;

use crate::app::{App, Preview, ROW_HEIGHT, SettingsTab, Sheet, SheetKind, home_dir, root_dir};
use crate::format;
use crate::icons;
use crate::keybinds::{Action, Keybinds};
use crate::settings::Settings;

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = theme::kit(cx.system_dark());
    let shell = SplitView::new(main(s, k))
        .sidebar(&s.sidebar, |s: &mut App| &mut s.sidebar, sidebar(s, k))
        .inspector(
            &s.inspector,
            |s: &mut App| &mut s.inspector,
            inspector(s, k),
        )
        .toolbar(toolbar(s, k))
        .titlebar_inset(cx.titlebar_inset())
        .sidebar_header(text("Sheath").font(k.ty.ui_medium).color(k.c.text_muted))
        .build(k);
    let mut layers = Vec::new();
    // The selection box a drag draws, over the rows and under any dialog.
    if let Some(box_rect) = s.marquee_rect() {
        layers.push(
            el().key("marquee")
                .absolute(box_rect[0], box_rect[1])
                .w(box_rect[2])
                .h(box_rect[3])
                .radius(3.0)
                .fill(k.alpha(k.c.text_accent, 0.12))
                .border(1.0, k.alpha(k.c.text_accent, 0.55)),
        );
    }
    // What a dragged selection is and where it is, following the pointer.
    if let Some((label, (x, y))) = s.drag_badge() {
        layers.push(
            el().key("drag-badge")
                .absolute(x + 14.0, y + 14.0)
                .radius(6.0)
                .px(10.0)
                .py(6.0)
                .fill(k.c.layer_02)
                .border(1.0, k.c.border_muted)
                .child(
                    text(label)
                        .font(k.ty.caption())
                        .color(k.c.text_base)
                        .max_lines(1),
                ),
        );
    }
    if s.palette.is_some() {
        palette_layer(s, k, &mut layers);
    }
    if let Some(at) = s.menu {
        row_menu(s, k, at, &mut layers);
    }
    if let Some(sheet) = &s.sheet {
        sheet_layer(s, sheet, k, &mut layers);
    }
    if s.settings.is_some() {
        settings_layer(s, k, cx.titlebar_inset(), &mut layers);
    }
    rune_kit::root(shell, layers)
}

fn toolbar(s: &App, k: &'static Kit) -> Toolbar {
    let back = nav_button(
        &icons::CHEVRON_LEFT,
        "Back",
        s.keybinds.accelerator(Action::Back),
        s.can_back(),
        |s: &mut App| s.back(),
        k,
    )
    .key("nav-back");
    let forward = nav_button(
        &icons::CHEVRON_RIGHT,
        "Forward",
        s.keybinds.accelerator(Action::Forward),
        s.can_forward(),
        |s: &mut App| s.forward(),
        k,
    )
    .key("nav-forward");
    let up = nav_button(
        &icons::ARROW_UP,
        "Enclosing Folder",
        s.keybinds.accelerator(Action::EnclosingFolder),
        s.can_up(),
        |s: &mut App| s.up(),
        k,
    )
    .key("nav-up");
    let search = rune_kit::TextInput::new(&s.search)
        .placeholder("Search this folder")
        .leading_icon(k.glyph.search)
        .clear_button()
        .width(Some(248.0))
        .on_edit(|s: &mut App| &mut s.search)
        .on_change(|s: &mut App, _| s.apply_filter())
        .on_action(|s: &mut App, _| {
            s.search.clear();
            s.apply_filter();
        })
        .build(k)
        .key("search");

    let new_folder = IconButton::new(&icons::FOLDER_PLUS)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("New Folder")
        .help("New Folder")
        .shortcut(s.keybinds.accelerator(Action::NewFolder))
        .on_press_tx(|s: &mut App, tx| s.new_folder(tx))
        .build(k)
        .key("new-folder");

    let (hidden_icon, hidden_label) = if s.show_hidden {
        (k.glyph.eye, "Hide Hidden Files")
    } else {
        (k.glyph.eye_off, "Show Hidden Files")
    };
    let hidden = IconButton::new(hidden_icon)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label(hidden_label)
        .help(hidden_label)
        .shortcut(s.keybinds.accelerator(Action::ToggleHidden))
        .expanded(s.show_hidden)
        .on_press(|s: &mut App| s.toggle_hidden())
        .build(k)
        .key("toggle-hidden");

    let inspector = IconButton::new(&icons::INSPECTOR)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("Toggle Inspector")
        .help("Toggle Inspector")
        .expanded(s.inspector.is_open())
        .on_press(|s: &mut App| s.toggle_inspector())
        .build(k)
        .key("toggle-inspector");

    let settings = IconButton::new(&theme::icons::SETTINGS_GEAR)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("Settings")
        .help("Settings")
        .shortcut(s.keybinds.accelerator(Action::OpenSettings))
        .on_press(|s: &mut App| s.open_settings())
        .build(k)
        .key("open-settings");

    Toolbar::new()
        .leading(back)
        .leading(forward)
        .leading(up)
        .center(search)
        .trailing(new_folder)
        .trailing(hidden)
        .trailing(inspector)
        .trailing(settings)
}

fn nav_button(
    glyph: &'static Icon,
    label: &'static str,
    hint: String,
    enabled: bool,
    action: impl Fn(&mut App) + 'static,
    k: &Kit,
) -> El {
    IconButton::new(glyph)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label(label)
        .help(label)
        .shortcut(hint)
        .disabled(!enabled)
        .on_press(action)
        .build(k)
}

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

fn sidebar(s: &App, k: &'static Kit) -> El {
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

fn main(s: &App, k: &'static Kit) -> El {
    let mut content = col()
        .key("browser")
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(path_bar(s, k));
    if let Some(error) = &s.error {
        content = content.child(
            row()
                .key("error")
                .w_full()
                .px(12.0)
                .py(8.0)
                .gap(8.0)
                .center_y()
                .fill(k.alpha(k.c.danger.bg, 0.5))
                .child(icon(&theme::icons::WARNING, 14.0).color(k.c.danger.fg))
                .child(
                    text(error.clone())
                        .font(k.ty.caption())
                        .color(k.c.danger.fg)
                        .grow(1.0)
                        .shrink()
                        .truncate()
                        .help(error.clone()),
                ),
        );
    }
    if s.visible.is_empty() {
        content.child(empty_state(s, k).grow(1.0).min_h(0.0))
    } else {
        content.child(file_table(s, k).grow(1.0).min_h(0.0))
    }
}

fn path_bar(s: &App, k: &'static Kit) -> El {
    let crumbs = crumbs(&s.cwd);
    let mut bar = row()
        .key("path-bar")
        .w_full()
        .h(40.0)
        .px(12.0)
        .gap(2.0)
        .center_y()
        .child(icon(&icons::DRIVE, 14.0).color(k.c.icon_faint));
    let last = crumbs.len().saturating_sub(1);
    let shown: Vec<usize> = if crumbs.len() <= 4 {
        (0..crumbs.len()).collect()
    } else {
        vec![0, crumbs.len() - 3, crumbs.len() - 2, crumbs.len() - 1]
    };
    let mut previous: Option<usize> = None;
    let mut first = true;
    for index in shown {
        if !first {
            bar = bar.child(icon(&icons::CHEVRON_RIGHT, 11.0).color(k.c.icon_faint));
        }
        if previous.is_some_and(|p| index > p + 1) {
            bar = bar.child(text("…").font(k.ty.ui).color(k.c.text_faint).px(4.0));
            bar = bar.child(icon(&icons::CHEVRON_RIGHT, 11.0).color(k.c.icon_faint));
        }
        let (name, path) = &crumbs[index];
        let label = shorten(name, 24);
        if index == last {
            bar = bar.child(
                text(label)
                    .font(k.ty.ui_medium)
                    .color(k.c.text_base)
                    .px(5.0)
                    .truncate(),
            );
        } else {
            let target = path.clone();
            bar = bar.child(
                text(label)
                    .font(k.ty.ui)
                    .color(k.c.text_muted)
                    .px(5.0)
                    .py(3.0)
                    .radius(k.radius.sm)
                    .truncate()
                    .hover(k.c.hover)
                    .cursor(Cursor::Pointer)
                    .help(path.display().to_string())
                    .on_press(move |s: &mut App| s.navigate(target.clone())),
            );
        }
        previous = Some(index);
        first = false;
    }
    bar = bar
        .child(el().key("path-spacer").grow(1.0))
        .child(
            text(s.phrase_for_count())
                .font(k.ty.caption())
                .color(k.c.text_faint),
        )
        .child(
            IconButton::new(&icons::REFRESH)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label("Refresh")
                .help("Refresh")
                .shortcut(s.keybinds.accelerator(Action::Refresh))
                .on_press(|s: &mut App| s.reload())
                .build(k)
                .key("refresh"),
        );
    col()
        .key("path-bar-wrap")
        .w_full()
        .child(bar)
        .child(el().key("path-rule").h(1.0).w_full().fill(k.c.border_muted))
}

fn crumbs(path: &std::path::Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut acc = PathBuf::new();
    for part in path.components() {
        acc.push(part.as_os_str());
        let name = match part {
            Component::RootDir => std::path::MAIN_SEPARATOR.to_string(),
            other => other.as_os_str().to_string_lossy().into_owned(),
        };
        out.push((name, acc.clone()));
    }
    if out.is_empty() {
        out.push((path.display().to_string(), path.to_path_buf()));
    }
    out
}

fn shorten(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_owned();
    }
    let keep = max.saturating_sub(1);
    let head = keep / 2;
    let tail = keep - head;
    let start: String = chars[..head].iter().collect();
    let end: String = chars[chars.len() - tail..].iter().collect();
    format!("{start}…{end}")
}

fn file_table(s: &App, k: &'static Kit) -> El {
    let style = TableStyle {
        row_height: ROW_HEIGHT,
        header_height: 32.0,
        ..TableStyle::from_kit(k)
    };
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
        .child(file_rows(s, k, style).offset(0.0, s.scroll_overshoot));
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
        let glyph = if direction == SortDir::Ascending {
            k.glyph.chevron_up
        } else {
            k.glyph.chevron_down
        };
        inner = inner.child(icon(glyph, 14.0).key("sort").color(style.text));
    }
    let mut cell = row()
        .role("columnheader")
        .h_full()
        .px(style.cell_pad)
        .center_y()
        .cursor(Cursor::Pointer)
        .hover(k.c.hover)
        .on_press(move |s: &mut App| s.sort_by(id))
        .child(inner);
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
        .growth(GrowthSpec::default())
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
        .center_y();
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

fn empty_state(s: &App, k: &'static Kit) -> El {
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

fn inspector(s: &App, k: &'static Kit) -> El {
    if s.selection_len() > 1 {
        return selection_inspector(s, k);
    }
    let Some(entry) = s.selected_entry() else {
        return col()
            .key("inspector")
            .w_full()
            .h_full()
            .px(20.0)
            .center()
            .gap(8.0)
            .child(icon(&glyphs::FILE, 40.0).color(k.c.icon_faint))
            .child(
                text("No selection")
                    .font(k.ty.ui_medium)
                    .color(k.c.text_muted),
            )
            .child(
                text("Select a file or folder to inspect it.")
                    .font(k.ty.caption())
                    .color(k.c.text_faint)
                    .align(Align::Center),
            );
    };
    let glyph = if entry.is_dir {
        &glyphs::FOLDER
    } else {
        file_icon(&entry.name)
    };
    let icon_color = if entry.is_dir {
        k.c.icon_accent
    } else {
        k.c.icon_muted
    };
    let parent = entry
        .path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut panel = col()
        .key("inspector")
        .w_full()
        .h_full()
        .min_h(0.0)
        .px(16.0)
        .py(14.0)
        .gap(12.0)
        .child(
            col()
                .w_full()
                .gap(3.0)
                .center()
                .child(icon(glyph, 52.0).color(icon_color))
                .child(
                    text(entry.name.clone())
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .align(Align::Center)
                        .max_lines(2)
                        .w_full(),
                )
                .child(
                    text(entry.kind())
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .align(Align::Center)
                        .w_full(),
                ),
        )
        .child(rule(k))
        .child(
            col()
                .key("info")
                .w_full()
                .gap(6.0)
                .child(info_row(
                    k,
                    "Size",
                    if entry.is_dir {
                        "—".into()
                    } else {
                        format::size(entry.size)
                    },
                ))
                .child(info_row(k, "Created", format::full_time(entry.created)))
                .child(info_row(k, "Modified", format::full_time(entry.modified)))
                .child(info_row(k, "Permissions", entry.permissions.clone()))
                .child(info_row_with_help(k, "Where", parent.clone(), parent)),
        )
        .child(actions(k, entry))
        .child(rule(k))
        .child(preview_section(s, k));
    if entry.readonly {
        panel = panel.child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon(k.glyph.lock, 12.0).color(k.c.text_faint))
                .child(text("Read-only").font(k.ty.caption()).color(k.c.text_faint)),
        );
    }
    panel
}

fn sheet_layer(s: &App, sheet: &Sheet, k: &Kit, layers: &mut Vec<El>) {
    match sheet.kind {
        SheetKind::Rename => prompt_sheet(
            sheet,
            k,
            layers,
            "rename",
            "Rename",
            "Choose a new name for the selected item.",
            "Rename",
            "Name",
            "Press Return to rename. Escape cancels.",
        ),
        SheetKind::Move => prompt_sheet(
            sheet,
            k,
            layers,
            "move",
            "Move",
            if sheet.many() {
                "Move the selected items into another folder."
            } else {
                "Move the selected item into another folder."
            },
            "Move",
            "Destination folder",
            "A folder path. Use ~ for your home folder.",
        ),
        SheetKind::Trash => trash_sheet(s, sheet, k, layers),
    }
}

#[allow(clippy::too_many_arguments)]
fn prompt_sheet(
    sheet: &Sheet,
    k: &Kit,
    layers: &mut Vec<El>,
    id: &str,
    title: &str,
    description: &str,
    action: &str,
    placeholder: &str,
    hint: &str,
) {
    let input = TextInput::new(&sheet.edit)
        .placeholder(placeholder)
        .width(None)
        .on_edit(|s: &mut App| s.sheet_edit())
        .on_submit(|s: &mut App, tx| s.submit_sheet(tx))
        .build(k)
        .key(format!("{id}-field"));
    let mut body = col()
        .key(format!("{id}-body"))
        .w_full()
        .px(16.0)
        .pb(16.0)
        .gap(10.0)
        .child(
            text(sheet.name.clone())
                .font(k.ty.ui_medium)
                .color(k.c.text_base)
                .max_lines(2)
                .w_full(),
        )
        .child(input)
        .child(text(hint).font(k.ty.caption()).color(k.c.text_faint).wrap());
    if let Some(error) = &sheet.error {
        body = body.child(error_text(k, error));
    }
    let cancel = Button::new("Cancel")
        .variant(ButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .on_press(|s: &mut App| s.close_sheet())
        .build(k)
        .key(format!("{id}-cancel"));
    let submit = Button::new(action)
        .variant(ButtonVariant::Contrast)
        .control_size(ControlSize::Regular)
        .on_press_tx(|s: &mut App, tx| s.submit_sheet(tx))
        .build(k)
        .key(format!("{id}-submit"))
        .default_button();
    body = body.child(
        row()
            .w_full()
            .justify_end()
            .gap(8.0)
            .child(cancel)
            .child(submit),
    );
    Dialog::new(id, title)
        .description(description)
        .width(400.0)
        .fit()
        .open(true)
        .child(body)
        .on_close(|s: &mut App| s.close_sheet())
        .build(k, layers);
}

fn trash_sheet(s: &App, sheet: &Sheet, k: &Kit, layers: &mut Vec<El>) {
    let mut body = col()
        .key("trash-body")
        .w_full()
        .px(16.0)
        .pb(16.0)
        .gap(10.0)
        .child(
            text(sheet.name.clone())
                .font(k.ty.ui_medium)
                .color(k.c.text_base)
                .max_lines(2)
                .w_full(),
        );
    if sheet.many() {
        body = body.child(selection_names(s, &sheet.targets, k));
    }
    if let Some(error) = &sheet.error {
        body = body.child(error_text(k, error));
    }
    let cancel = Button::new("Cancel")
        .variant(ButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .on_press(|s: &mut App| s.close_sheet())
        .build(k)
        .key("trash-cancel")
        .default_button();
    let confirm = Button::new("Move to Trash")
        .variant(ButtonVariant::Danger)
        .icon(&theme::icons::TRASH)
        .control_size(ControlSize::Regular)
        .on_press_tx(|s: &mut App, tx| s.submit_sheet(tx))
        .build(k)
        .key("trash-submit");
    body = body.child(
        row()
            .w_full()
            .justify_end()
            .gap(8.0)
            .child(cancel)
            .child(confirm),
    );
    Dialog::new("trash", "Move to Trash?")
        .description(if sheet.many() {
            "The items will move to the system Trash."
        } else {
            "The item will move to the system Trash."
        })
        .width(400.0)
        .fit()
        .open(true)
        .child(body)
        .on_close(|s: &mut App| s.close_sheet())
        .build(k, layers);
}

fn error_text(k: &Kit, error: &str) -> El {
    text(error.to_owned())
        .font(k.ty.caption())
        .color(k.c.danger.fg)
        .wrap()
}

/// The inspector for a selection of several rows: what they add up to, instead of one file's
/// details and preview.
fn selection_inspector(s: &App, k: &Kit) -> El {
    let count = s.selection_len();
    let folders = s.selection_folders();
    let files = count - folders;
    let size = s.selection_size();
    col()
        .key("inspector")
        .w_full()
        .h_full()
        .min_h(0.0)
        .px(16.0)
        .py(14.0)
        .gap(12.0)
        .child(
            col()
                .key("selection-summary")
                .w_full()
                .gap(3.0)
                .center()
                .child(icon(&glyphs::FOLDER, 52.0).color(k.c.icon_accent))
                .child(
                    text(format!("{count} items selected"))
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .align(Align::Center)
                        .w_full(),
                )
                .child(
                    text("Move and Trash act on all of them.")
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .align(Align::Center)
                        .w_full(),
                ),
        )
        .child(rule(k))
        .child(
            col()
                .key("info")
                .w_full()
                .gap(6.0)
                .child(info_row(k, "Items", count.to_string()))
                .child(info_row(
                    k,
                    "Folders",
                    if folders == 0 {
                        "—".into()
                    } else {
                        folders.to_string()
                    },
                ))
                .child(info_row(
                    k,
                    "Files",
                    if files == 0 {
                        "—".into()
                    } else {
                        files.to_string()
                    },
                ))
                .child(info_row(
                    k,
                    "Size",
                    if files == 0 {
                        "—".into()
                    } else {
                        format::size(size)
                    },
                )),
        )
        .child(rule(k))
        .child(
            text("Select one item to inspect it. Rename works on one item at a time.")
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .wrap(),
        )
}

/// Up to five names, so a confirmation for a selection of several says what it covers.
fn selection_names(s: &App, ids: &[String], k: &Kit) -> El {
    let names: Vec<&str> = ids.iter().filter_map(|id| s.entry_name(id)).collect();
    let shown = names.len().min(5);
    let mut list = names[..shown].join("\n");
    if names.len() > shown {
        list.push_str(&format!("\nand {} more", names.len() - shown));
    }
    text(list).font(k.ty.caption()).color(k.c.text_muted).wrap()
}

fn rule(k: &Kit) -> El {
    el().h(1.0).w_full().fill(k.c.border_muted)
}

fn info_row(k: &Kit, label: &str, value: String) -> El {
    info_row_with_help(k, label, value, String::new())
}

fn info_row_with_help(k: &Kit, label: &str, value: String, help: String) -> El {
    let mut value_el = text(value)
        .font(k.ty.caption())
        .color(k.c.text_muted)
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .truncate();
    if !help.is_empty() {
        value_el = value_el.help(help);
    }
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text(label)
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .w(66.0),
        )
        .child(value_el)
}

fn actions(k: &Kit, entry: &crate::app::Entry) -> El {
    let path = entry.path.to_string_lossy().into_owned();
    row()
        .key("actions")
        .w_full()
        .gap(8.0)
        .child(
            Button::new("Open")
                .variant(ButtonVariant::Contrast)
                .control_size(ControlSize::Regular)
                .on_press(|s: &mut App| s.open_selected())
                .build(k)
                .grow(1.0),
        )
        .child(
            Button::new("Copy Path")
                .variant(ButtonVariant::Outline)
                .control_size(ControlSize::Regular)
                .on_press_tx(move |_: &mut App, tx: &mut Tx| tx.copy(path.clone()))
                .build(k),
        )
}

/// The row menu a right-click opens: the file operations, at the press point. A selection of
/// several rows moves or trashes all of them, so the labels say how many.
fn row_menu(s: &App, k: &Kit, at: PointerPress, layers: &mut Vec<El>) {
    let count = s.selection_len();
    let many = count > 1;
    context_menu("row-menu", el(), Some(at))
        .item(
            // One name at a time: the sheet renames a single row.
            MenuItem::new("Rename")
                .icon(&theme::icons::EDIT)
                .shortcut(s.keybinds.menu_label(Action::Rename))
                .disabled(many)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.begin_rename();
                }),
        )
        .item(
            MenuItem::new(if many {
                format!("Move {count} Items…")
            } else {
                "Move to Folder…".to_owned()
            })
            .icon(&glyphs::FOLDER)
            .shortcut(s.keybinds.menu_label(Action::Move))
            .on_select(|s: &mut App| {
                s.close_menu();
                s.begin_move();
            }),
        )
        .separator()
        .item(
            MenuItem::new(if many {
                format!("Move {count} Items to Trash")
            } else {
                "Move to Trash".to_owned()
            })
            .icon(&theme::icons::TRASH)
            .shortcut(s.keybinds.menu_label(Action::Trash))
            .on_select(|s: &mut App| {
                s.close_menu();
                s.begin_trash();
            }),
        )
        .on_close(|s: &mut App| s.close_menu())
        .build(k, layers);
}

/// The settings panel: a sidebar of categories with the chosen one beside it. It covers the
/// window and gives Escape back to the browser.
fn settings_layer(s: &App, k: &Kit, titlebar: f32, layers: &mut Vec<El>) {
    let Some(tab) = s.settings else {
        return;
    };
    let mut categories = SidebarSection::new("Categories").key("settings-categories");
    for category in SettingsTab::ALL {
        categories = categories.row(
            SidebarRow::new(category.label())
                .icon(tab_icon(category))
                .selected(category == tab)
                .on_select(move |s: &mut App, _| s.open_tab(category))
                .build(k)
                .key(format!("settings-tab-{}", category.id())),
        );
    }
    let sidebar = Sidebar::new()
        .inset(10.0)
        .row_gap(1.0)
        .section(categories)
        .build(k);
    let done = Button::new("Done")
        .variant(ButtonVariant::Contrast)
        .control_size(ControlSize::Regular)
        .on_press(|s: &mut App| s.close_settings())
        .build(k)
        .key("close-settings")
        .default_button();
    let page = match tab {
        SettingsTab::General => general_settings(s, k),
        SettingsTab::Keybinds => keybinds_settings(s, k),
        SettingsTab::About => about_settings(k),
    };
    layers.push(
        col()
            .key("settings")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .fill(k.c.bg_base)
            .modal(false, |s: &mut App| s.close_settings())
            // The panel is opaque, so it takes presses itself: none reach the browser below.
            .on_press(|_: &mut App| {})
            .child(
                row()
                    .key("settings-header")
                    .w_full()
                    .h(56.0)
                    .pl(titlebar.max(16.0))
                    .pr(16.0)
                    .gap(12.0)
                    .center_y()
                    .child(
                        text("Settings")
                            .font(k.ty.ui_medium)
                            .color(k.c.text_base)
                            .grow(1.0),
                    )
                    .child(done),
            )
            .child(
                el().key("settings-rule")
                    .h(1.0)
                    .w_full()
                    .fill(k.c.border_muted),
            )
            .child(
                row()
                    .w_full()
                    .grow(1.0)
                    .min_h(0.0)
                    .child(
                        col()
                            .key("settings-sidebar")
                            .w(200.0)
                            .h_full()
                            .fill(k.c.layer_01)
                            .child(sidebar),
                    )
                    .child(
                        el().key("settings-divider")
                            .w(1.0)
                            .h_full()
                            .fill(k.c.border_muted),
                    )
                    .child(
                        col()
                            .key("settings-content")
                            .grow(1.0)
                            .h_full()
                            .min_h(0.0)
                            .child(page),
                    ),
            ),
    );
}

fn tab_icon(tab: SettingsTab) -> &'static Icon {
    match tab {
        SettingsTab::General => &theme::icons::SETTINGS_GEAR,
        SettingsTab::Keybinds => &glyphs::KEYBOARD,
        SettingsTab::About => &theme::icons::INFO,
    }
}

/// The command bar: a field over the registry's actions, each with its shortcut.
fn palette_layer(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(palette) = &s.palette else {
        return;
    };
    let actions = s.palette_actions();
    let field = TextInput::new(&palette.query)
        .placeholder("Type a command")
        .leading_icon(k.glyph.search)
        .width(None)
        .on_edit(|s: &mut App| s.palette_edit())
        .on_change(|s: &mut App, _| s.palette_requery())
        .on_submit(|s: &mut App, tx| s.run_palette_command(tx))
        .on_key(|s: &mut App, event, tx| {
            // The field owns the keyboard here, so the bar's keys are the ones it leaves over.
            if event.key == Key::Escape {
                s.close_palette();
                return true;
            }
            if s.keybinds.matches(Action::CommandBar, event) {
                s.close_palette();
                return true;
            }
            if event.mods.cmd || event.mods.ctrl || event.mods.alt {
                return false;
            }
            match event.key {
                Key::Down => s.palette_move(1),
                Key::Up => s.palette_move(-1),
                Key::Enter if !tx.key_repeat() => s.run_palette_command(tx),
                _ => return false,
            }
            true
        })
        .build(k)
        .key("command-field");
    // The padding lives on the list, not on each row: a row that is `w_full` and also has
    // margins would be wider than the bar and clip at its right edge.
    let mut list = col().key("command-list").w_full().gap(1.0).px(6.0).py(6.0);
    if actions.is_empty() {
        list = list.child(
            row().key("command-none").px(12.0).py(14.0).child(
                text("No commands match")
                    .font(k.ty.ui)
                    .color(k.c.text_faint),
            ),
        );
    }
    for (i, action) in actions.iter().copied().enumerate() {
        let enabled = s.action_enabled(action);
        let selected = i == palette.selected;
        let mut row = row()
            .key(format!("command-{}", action.id()))
            .w_full()
            .h(32.0)
            .px(8.0)
            .gap(10.0)
            .center_y()
            .radius(k.radius.sm)
            .child(icon(action_icon(action, k), 14.0).color(if enabled {
                k.c.icon_muted
            } else {
                k.c.icon_faint
            }))
            .child(
                text(action.label())
                    .font(k.ty.ui)
                    .color(if enabled {
                        k.c.text_base
                    } else {
                        k.c.text_faint
                    })
                    .grow(1.0)
                    .truncate(),
            )
            .child(
                text(s.keybinds.menu_label(action))
                    .font(k.ty.caption())
                    .color(k.c.text_faint),
            );
        if selected {
            row = row.fill(k.c.hover);
        }
        if enabled {
            row = row
                .cursor(Cursor::Pointer)
                .on_press_tx(move |s: &mut App, tx: &mut Tx| s.choose_command(action, tx));
        }
        list = list.child(row);
    }
    let panel = col()
        .key("command-bar")
        .w(560.0)
        .max_h(440.0)
        .radius(10.0)
        .fill(k.c.layer_01)
        .border(1.0, k.c.border_muted)
        .clip_rounded(10.0)
        // A modal, so Escape closes it (rather than just blurring the field) and focus stays
        // inside while it's up.
        .modal(true, |s: &mut App| s.close_palette())
        .child(row().px(10.0).py(8.0).child(field))
        .child(
            el().key("command-rule")
                .h(1.0)
                .w_full()
                .fill(k.c.border_muted),
        )
        .child(
            col()
                .key("command-scroll")
                .w_full()
                .max_h(380.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_edge_fade(12.0)
                .child(list),
        );
    layers.push(
        scrim("command-scrim", |s: &mut App| s.close_palette()).fill(k.alpha(k.c.black, 0.28)),
    );
    layers.push(
        col()
            .key("command-slot")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .pt(72.0)
            .child(row().w_full().justify_center().child(panel)),
    );
}

fn action_icon(action: Action, k: &Kit) -> &'static Icon {
    match action {
        Action::CommandBar => &glyphs::KEYBOARD,
        Action::OpenSettings => &theme::icons::SETTINGS_GEAR,
        Action::Back => &icons::CHEVRON_LEFT,
        Action::Forward => &icons::CHEVRON_RIGHT,
        Action::EnclosingFolder => &icons::ARROW_UP,
        Action::Refresh => &icons::REFRESH,
        Action::NewFolder => &icons::FOLDER_PLUS,
        Action::ToggleHidden => k.glyph.eye,
        Action::Rename => &theme::icons::EDIT,
        Action::Move => &glyphs::FOLDER,
        Action::Trash => &theme::icons::TRASH,
        Action::Copy => &theme::icons::COPY,
        Action::Cut => &icons::SCISSORS,
        Action::Paste => &icons::CLIPBOARD,
        Action::Duplicate => &theme::icons::OUTLINE_COPY,
    }
}

fn general_settings(s: &App, k: &Kit) -> El {
    let hidden = Switch::new(s.show_hidden)
        .label("Show hidden files")
        .hide_label()
        .control_size(ControlSize::Regular)
        .on_toggle(|s: &mut App| s.toggle_hidden())
        .build(k)
        .key("setting-hidden");
    let inspector = Switch::new(s.inspector.is_open())
        .label("Show inspector")
        .hide_label()
        .control_size(ControlSize::Regular)
        .on_toggle(|s: &mut App| s.toggle_inspector())
        .build(k)
        .key("setting-inspector");
    let inertia = Switch::new(s.config.scroll_inertia)
        .label("Scroll inertia")
        .hide_label()
        .control_size(ControlSize::Regular)
        .on_toggle(|s: &mut App| s.toggle_scroll_inertia())
        .build(k)
        .key("setting-inertia");
    let overscroll = Switch::new(s.config.overscroll)
        .label("Overscroll")
        .hide_label()
        .control_size(ControlSize::Regular)
        .on_toggle(|s: &mut App| s.toggle_overscroll())
        .build(k)
        .key("setting-overscroll");
    let mut page = col()
        .w_full()
        .px(24.0)
        .py(20.0)
        .gap(14.0)
        .child(page_title(
            k,
            "General",
            "How the browser presents a folder and scrolls its list. These are saved in config.kdl.",
        ))
        .child(setting_row(
            k,
            "Show hidden files",
            "Also list names that start with a dot.",
            hidden,
        ))
        .child(rule(k))
        .child(setting_row(
            k,
            "Show inspector",
            "The details and preview pane beside the table.",
            inspector,
        ))
        .child(rule(k))
        .child(setting_row(
            k,
            "Scroll inertia",
            "Keep the list coasting after the wheel stops.",
            inertia,
        ))
        .child(rule(k))
        .child(setting_row(
            k,
            "Overscroll",
            "Pull the list past its first and last row, then let it spring back.",
            overscroll,
        ));
    if let Some(error) = &s.config.error {
        page = page.child(error_text(k, error));
    }
    col()
        .key("settings-general")
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(page)
}

fn keybinds_settings(s: &App, k: &Kit) -> El {
    let reset = Button::new("Restore Defaults")
        .variant(ButtonVariant::GhostMuted)
        .control_size(ControlSize::Small)
        .on_press(|s: &mut App| s.reset_bindings())
        .build(k)
        .key("reset-keybinds");
    let mut list = col().key("keybind-list").w_full().gap(0.0);
    for action in Action::ALL {
        let Some(state) = s.recorders.get(&action) else {
            continue;
        };
        let recorder = ShortcutRecorder::new(
            format!("record-{}", action.id()),
            state,
            move |s: &mut App| {
                s.recorders
                    .get_mut(&action)
                    .expect("every action has a recorder")
            },
        )
        .width(196.0)
        .allow_unmodified()
        .on_change(move |s: &mut App, shortcut, _: &mut Tx| s.set_binding(action, shortcut))
        .build(k);
        list = list
            .child(
                row()
                    .w_full()
                    .gap(12.0)
                    .center_y()
                    .py(6.0)
                    .child(
                        text(action.label())
                            .font(k.ty.ui)
                            .color(k.c.text_base)
                            .grow(1.0)
                            .truncate(),
                    )
                    .child(recorder),
            )
            .child(rule(k));
    }
    let saved = match Keybinds::file() {
        Some(path) => format!(
            "Click a shortcut and press the keys to use instead. Backspace clears it, Escape \
             cancels while recording. Saved in {}.",
            path.display()
        ),
        None => "Click a shortcut and press the keys to use instead. Backspace clears it, \
                 Escape cancels while recording."
            .to_owned(),
    };
    let mut header = col()
        .key("keybind-header")
        .w_full()
        .px(24.0)
        .pt(20.0)
        .pb(12.0)
        .gap(10.0)
        .child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .child(page_title(k, "Keybinds", &saved).grow(1.0))
                .child(reset),
        );
    if let Some(error) = &s.keybinds.error {
        header = header.child(error_text(k, error));
    }
    col()
        .key("settings-keybinds")
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(header)
        .child(
            col()
                .key("keybind-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_edge_fade(16.0)
                .child(col().w_full().px(24.0).pb(20.0).child(list)),
        )
}

fn about_settings(k: &Kit) -> El {
    let keybinds = file_path_row(k, "Keybinds", Keybinds::file(), "copy-keybinds-path");
    let settings = file_path_row(k, "Settings", Settings::file(), "copy-settings-path");
    col()
        .key("settings-about")
        .w_full()
        .h_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            col()
                .w_full()
                .px(24.0)
                .py(20.0)
                .gap(14.0)
                .child(page_title(
                    k,
                    "About",
                    "Sheath is a file manager proof of concept: every pixel is drawn by Rune, from \
                     its layout and input layers to the file table and this panel.",
                ))
                .child(rule(k))
                .child(
                    col()
                        .key("about-info")
                        .w_full()
                        .gap(6.0)
                        .child(info_row(k, "Version", env!("CARGO_PKG_VERSION").into()))
                        .child(keybinds)
                        .child(settings),
                )
                .child(
                    text(
                        "Keybinds and settings each live in their own file so they can be edited \
                         by hand, and editing one never rewrites the other.",
                    )
                    .font(k.ty.caption())
                    .color(k.c.text_faint)
                    .wrap(),
                ),
        )
}

/// A file path with a button that copies it, when the file has a path.
fn file_path_row(k: &Kit, label: &str, path: Option<PathBuf>, key: &str) -> El {
    let shown = path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "Unavailable".into());
    let info = info_row(k, label, shown.clone());
    let Some(_) = path else {
        return info;
    };
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(info.grow(1.0))
        .child(
            Button::new("Copy Path")
                .variant(ButtonVariant::Outline)
                .control_size(ControlSize::Small)
                .on_press_tx(move |_: &mut App, tx: &mut Tx| tx.copy(shown.clone()))
                .build(k)
                .key(key),
        )
}

fn page_title(k: &Kit, title: &str, description: &str) -> El {
    col()
        .w_full()
        .gap(4.0)
        .child(text(title).font(k.ty.ui_medium).color(k.c.text_base))
        .child(
            text(description)
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .wrap(),
        )
}

fn setting_row(k: &Kit, label: &str, description: &str, control: El) -> El {
    row()
        .w_full()
        .gap(16.0)
        .center_y()
        .py(4.0)
        .child(
            col()
                .gap(2.0)
                .grow(1.0)
                .min_w(0.0)
                .child(text(label).font(k.ty.ui).color(k.c.text_base))
                .child(
                    text(description)
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .wrap(),
                ),
        )
        .child(control)
}

fn preview_section(s: &App, k: &'static Kit) -> El {
    let mut panel = col()
        .key("preview")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(6.0)
        .child(
            text("Preview")
                .font(k.ty.small_medium)
                .color(k.c.text_muted),
        );
    match &s.preview {
        Some(Preview::Text(body)) if body.is_empty() => {
            panel = panel.child(note(k, "Empty file"));
        }
        Some(Preview::Text(body)) => {
            let body: String = body.chars().take(6_000).collect();
            panel = panel.child(
                col()
                    .key("preview-scroll")
                    .w_full()
                    .grow(1.0)
                    .min_h(0.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .scroll_edge_fade(12.0)
                    .child(
                        el().w_full()
                            .padding(10.0)
                            .radius(k.radius.md)
                            .fill(k.c.layer_02)
                            .child(
                                text(body)
                                    .font(k.ty.mono.lh(16.0))
                                    .color(k.c.text_muted)
                                    .wrap_anywhere(),
                            ),
                    ),
            );
        }
        Some(Preview::Folder(count)) => {
            let label = if *count == 1 {
                "1 item".to_owned()
            } else {
                format!("{count} items")
            };
            panel = panel.child(note(k, label));
        }
        Some(Preview::TooLarge) => {
            panel = panel.child(note(k, "Too large to preview · 24 KB limit"))
        }
        Some(Preview::Binary) => panel = panel.child(note(k, "No text preview")),
        None => panel = panel.child(note(k, "No preview")),
    }
    panel
}

fn note(k: &Kit, body: impl Into<String>) -> El {
    text(body)
        .font(k.ty.caption())
        .color(k.c.text_faint)
        .py(4.0)
        .wrap()
}
