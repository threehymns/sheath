use crate::app::App;
use crate::icons;
use crate::keybinds::Action;
use rune::{Cursor, El, col, el, icon, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit};
use std::path::PathBuf;

use super::*;

pub(super) fn path_bar(s: &App, k: &'static Kit) -> El {
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
