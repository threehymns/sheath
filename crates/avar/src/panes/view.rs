//! How a tab's blocks are drawn: a card each, with its folder and the buttons that split, zoom
//! and close it, and the gaps between cards that the pointer drags to resize them.

use rune::{Color, Cursor, Drag, El, Phase, Tx, col, el, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit};
use rune_tty::{layout_screen, screen_of, terminal_cell, terminal_drag};

use super::{Block, GAP, Node, Panes, Side};
use crate::Window;
use crate::icons;

/// A block's header: its folder on one side, its buttons on the other.
const HEADER: f32 = 28.0;
/// A card's corner, and the radius its grid is clipped to.
const RADIUS: f32 = 10.0;

/// A tab's layout: cards divided by the gaps between them, each split nested in the last.
pub fn tree(p: &Panes, tab: u64, node: &Node, scale: f32, k: &'static Kit) -> El {
    match node {
        Node::Block(id) => match p.block(*id) {
            Some(block) => card(p, tab, block, scale, k),
            None => rune::el(),
        },
        Node::Split {
            id,
            side,
            share,
            first,
            second,
            ..
        } => {
            let split = *id;
            let side = *side;
            let share = *share;
            let first = tree(p, tab, first, scale, k);
            let second = tree(p, tab, second, scale, k);
            let divider = divider(tab, split, side, k);
            let column = side == Side::Right;
            let mut container = if column {
                row()
                    .key(format!("split-{split}"))
                    .w_full()
                    .h_full()
                    .min_h(0.0)
            } else {
                col()
                    .key(format!("split-{split}"))
                    .w_full()
                    .h_full()
                    .min_w(0.0)
            };
            container = container.on_layout_tx(move |s: &mut Window, rect, _: &mut Tx| {
                // The split's own extent along its axis: what a drag divides points by.
                if let Some(p) = s.tabs.panes_mut(tab) {
                    p.set_length(split, if column { rect[2] } else { rect[3] });
                }
            });
            let a = if column {
                col()
                    .key(format!("split-{split}-first"))
                    .grow(share)
                    .min_w(0.0)
                    .h_full()
                    .child(first)
            } else {
                col()
                    .key(format!("split-{split}-first"))
                    .grow(share)
                    .w_full()
                    .min_h(0.0)
                    .child(first)
            };
            let b = if column {
                col()
                    .key(format!("split-{split}-second"))
                    .grow(1.0 - share)
                    .min_w(0.0)
                    .h_full()
                    .child(second)
            } else {
                col()
                    .key(format!("split-{split}-second"))
                    .grow(1.0 - share)
                    .w_full()
                    .min_h(0.0)
                    .child(second)
            };
            container.child(a).child(divider).child(b)
        }
    }
}

/// The gap between two blocks: drag it to resize, double-click it to put the split back to half.
fn divider(tab: u64, split: u64, side: Side, k: &'static Kit) -> El {
    let column = side == Side::Right;
    let mut grip = el()
        .key(format!("divider-{split}"))
        .role("separator")
        .label("Resize panes")
        .fill(Color::TRANSPARENT)
        .hover(k.c.hover)
        .cursor(if column {
            Cursor::ResizeColumn
        } else {
            Cursor::ResizeRow
        })
        .transition(rune::Transition::none());
    grip = if column {
        grip.w(GAP).h_full()
    } else {
        grip.h(GAP).w_full()
    };
    grip.on_drag(move |s: &mut Window, d: &mut Drag, tx: &mut Tx| {
        let Some(p) = s.tabs.panes_mut(tab) else {
            return;
        };
        match d.phase {
            Phase::Down => {
                tx.instant();
                p.begin_resize(split);
            }
            Phase::Move => {
                tx.instant();
                p.drag_resize(split, if column { d.dx } else { d.dy });
            }
            Phase::Up => p.end_resize(),
        }
    })
    .on_double_press(move |s: &mut Window| {
        if let Some(p) = s.tabs.panes_mut(tab) {
            p.reset_split(split);
        }
    })
}

/// A tab's only block, with no card around it: the window opens as a terminal, so a fresh one
/// shows a prompt and nothing else. The chrome arrives with the second pane or the second tab.
pub fn bare(tab: u64, block: &Block, scale: f32, k: &'static Kit) -> El {
    let id = block.id;
    col()
        .key(format!("block-{id}"))
        .w_full()
        .h_full()
        .min_h(0.0)
        .clip_rounded(0.0)
        .on_layout_tx(move |s: &mut Window, rect, _: &mut Tx| {
            if let Some(p) = s.tabs.panes_mut(tab) {
                p.set_rect(id, rect);
            }
        })
        .child(grid(block, tab, scale, k))
}

/// One block: a card over the window's background, its folder in the header and its shell under
/// it. The card that has the keyboard is the one drawn as active.
pub fn card(p: &Panes, tab: u64, block: &Block, scale: f32, k: &'static Kit) -> El {
    let id = block.id;
    let focused = p.focuses(id);
    let name = folder(block);
    col()
        .key(format!("block-{id}"))
        .w_full()
        .h_full()
        .min_h(0.0)
        .radius(RADIUS)
        .fill(k.c.bg_base)
        .border(
            1.0,
            if focused {
                k.c.border_muted
            } else {
                k.c.border_weak
            },
        )
        .clip_rounded(RADIUS)
        .on_layout_tx(move |s: &mut Window, rect, _: &mut Tx| {
            if let Some(p) = s.tabs.panes_mut(tab) {
                p.set_rect(id, rect);
            }
        })
        .child(header(tab, block, &name, focused, k))
        .child(grid(block, tab, scale, k))
}

fn header(tab: u64, block: &Block, name: &str, focused: bool, k: &'static Kit) -> El {
    let id = block.id;
    let button = |glyph: &'static rune::Icon, label: &'static str| {
        IconButton::new(glyph)
            .variant(IconButtonVariant::GhostMuted)
            .control_size(ControlSize::Mini)
            .label(label)
            .help(label)
    };
    let split_right = button(&icons::SPLIT_RIGHT, "Split right")
        .on_press(move |s: &mut Window| s.tabs.split_at(tab, id, Side::Right))
        .build(k)
        .key(format!("block-{id}-split-right"));
    let split_down = button(&icons::SPLIT_DOWN, "Split down")
        .on_press(move |s: &mut Window| s.tabs.split_at(tab, id, Side::Down))
        .build(k)
        .key(format!("block-{id}-split-down"));
    let zoom = button(&icons::ZOOM, "Zoom pane")
        .on_press(move |s: &mut Window| s.tabs.zoom_at(tab, id))
        .build(k)
        .key(format!("block-{id}-zoom"));
    let close = button(&rune_kit::glyphs::CLOSE, "Close pane")
        .on_press(move |s: &mut Window| s.tabs.close_block(tab, id))
        .build(k)
        .key(format!("block-{id}-close"));
    row()
        .key(format!("block-{id}-header"))
        .w_full()
        .h(HEADER)
        .px(8.0)
        .gap(2.0)
        .center_y()
        .child(
            text(name.to_owned())
                .key(format!("block-{id}-name"))
                .font(k.ty.small)
                .color(if focused {
                    k.c.text_base
                } else {
                    k.c.text_muted
                })
                .grow(1.0)
                .truncate(),
        )
        .child(split_right)
        .child(split_down)
        .child(zoom)
        .child(close)
}

/// One block's grid, with its wheel, pointer, drag and layout wired to that block's shell.
fn grid(block: &Block, tab: u64, scale: f32, k: &'static Kit) -> El {
    let id = block.id;
    let screen = screen_of(&block.term, scale, k)
        .on_wheel(move |s: &mut Window, wheel, tx: &mut Tx| {
            // The same split the app's pane makes: a program that captured the mouse scrolls
            // its own view, and shift takes the mouse back for the history.
            let Some(term) = s.tabs.block_mut(tab, id) else {
                return false;
            };
            let shift = tx.mods().shift;
            if !shift
                && let Some(at) = term.hovered_cell()
                && term.mouse_wheel(wheel.dy > 0.0, at, false)
            {
                return true;
            }
            term.wheel(wheel.dy)
        })
        .on_pointer_move_local(move |s: &mut Window, p, tx: &mut Tx| {
            if let Some(term) = s.tabs.block_mut(tab, id) {
                let at = terminal_cell(term, (p.x, p.y));
                term.hover(at, tx.mods().shift);
            }
        })
        .on_hover_change(move |s: &mut Window, inside: bool, _: &mut Tx| {
            if !inside && let Some(term) = s.tabs.block_mut(tab, id) {
                term.hover(None, false);
            }
        })
        .on_drag(move |s: &mut Window, drag: &mut Drag, tx: &mut Tx| {
            if let Some(term) = s.tabs.block_mut(tab, id) {
                terminal_drag(term, drag, tx);
            }
        })
        // A press anywhere in the grid gives that block the keyboard, and its tab the window.
        .on_press(move |s: &mut Window| s.tabs.focus_block(tab, id))
        .on_layout_tx(move |s: &mut Window, rect, tx: &mut Tx| {
            if let Some(term) = s.tabs.block_mut(tab, id) {
                layout_screen(term, rect);
            }
            tx.live_only();
        });
    el().key(format!("grid-{id}"))
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .clip_rounded(0.0)
        .child(screen)
}

/// The folder a block's shell is in, as its header names it.
fn folder(block: &Block) -> String {
    let Some(dir) = block.term.pwd() else {
        return "Terminal".to_owned();
    };
    dir.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| dir.display().to_string())
}
