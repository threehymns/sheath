//! How a tab's blocks are drawn: the shell of a pane each, the header that names it and splits,
//! zooms and closes it when the settings ask for one, and the gaps between panes that the
//! pointer drags to resize them.

use rune::{Color, Cursor, Drag, El, Phase, Spring, Tx, col, el, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit};
use rune_tty::{layout_screen, screen_of, terminal_cell, terminal_drag};

use super::{Block, GAP, HEADER, Node, Panes, Side};
use crate::Window;
use crate::icons;

/// A tab's layout: panes divided by the gaps between them, each split nested in the last. With
/// `zoom` set, every split on that block's path gives it the whole share and the other side is
/// drawn empty, so a zoom is a resize of one pane rather than a swap of the whole tab.
pub fn tree(
    p: &Panes,
    tab: u64,
    node: &Node,
    scale: f32,
    k: &'static Kit,
    headers: bool,
    zoom: Option<u64>,
) -> El {
    match node {
        Node::Block(id) => match p.block(*id) {
            Some(block) => pane(p, tab, block, scale, k, headers),
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
            let column = side == Side::Right;
            let first_holds = zoom.is_some_and(|focus| holds(first, focus));
            let second_holds = zoom.is_some_and(|focus| holds(second, focus));
            let zoomed = first_holds || second_holds;
            // Zoomed, the focused pane takes the whole split and the other side is empty, so
            // the pane widens (or narrows) on the layout spring instead of flying there with
            // its text scaled. Otherwise the split keeps the shares it was left with.
            let (first_share, second_share) = match zoom {
                Some(_) if first_holds => (1.0, 0.0),
                Some(_) if second_holds => (0.0, 1.0),
                _ => (*share, 1.0 - *share),
            };
            let first = if zoomed && !first_holds {
                rune::el()
            } else {
                tree(p, tab, first, scale, k, headers, zoom)
            };
            let second = if zoomed && !second_holds {
                rune::el()
            } else {
                tree(p, tab, second, scale, k, headers, zoom)
            };
            let divider = divider(tab, split, side, k);
            // A share moved by a command (a resize step, a balance) glides on the kit's layout
            // spring instead of jumping; a divider drag lands at once through `tx.instant`, so
            // the gap still follows the pointer.
            let mut container = if column {
                row()
                    .key(format!("split-{split}"))
                    .w_full()
                    .h_full()
                    .min_h(0.0)
                    .motion(k.motion.disclosure)
            } else {
                col()
                    .key(format!("split-{split}"))
                    .w_full()
                    .h_full()
                    .min_w(0.0)
                    .motion(k.motion.disclosure)
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
                    .grow(first_share)
                    .min_w(0.0)
                    .h_full()
                    .child(first)
            } else {
                col()
                    .key(format!("split-{split}-first"))
                    .grow(first_share)
                    .w_full()
                    .min_h(0.0)
                    .child(first)
            };
            let b = if column {
                col()
                    .key(format!("split-{split}-second"))
                    .grow(second_share)
                    .min_w(0.0)
                    .h_full()
                    .child(second)
            } else {
                col()
                    .key(format!("split-{split}-second"))
                    .grow(second_share)
                    .w_full()
                    .min_h(0.0)
                    .child(second)
            };
            // A zoomed split has no divider: its whole extent goes to the pane on the path.
            container = container.child(a);
            if !zoomed {
                container = container.child(divider);
            }
            container.child(b)
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

/// A tab's only block, with no chrome around it: the window opens as a terminal, so a fresh one
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

/// One block: its shell, and the header naming it when the settings ask for headers. The pane
/// sits on the window's own background, with no card of its own, so a split reads as the
/// terminal the window opened as.
pub fn pane(p: &Panes, tab: u64, block: &Block, scale: f32, k: &'static Kit, headers: bool) -> El {
    let id = block.id;
    let mut body = col()
        .key(format!("block-{id}"))
        .w_full()
        .h_full()
        .min_h(0.0)
        .motion(k.motion.disclosure)
        .clip_rounded(0.0)
        .on_layout_tx(move |s: &mut Window, rect, _: &mut Tx| {
            if let Some(p) = s.tabs.panes_mut(tab) {
                p.set_rect(id, rect);
            }
        });
    if headers {
        let name = p.name(id);
        let header = header(tab, block, &name, p.focuses(id), p.is_zoomed(), k);
        body = body.child(header);
    }
    body.child(grid(block, tab, scale, k))
}

/// A block's header, which also drags: pressing it picks the pane up, letting go over another
/// pane's body swaps the two, and letting go on one of its edges splits beside it. The drag
/// previews the layout itself, so nothing here marks where a drop would land.
fn header(tab: u64, block: &Block, name: &str, focused: bool, zoomed: bool, k: &'static Kit) -> El {
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
    // The block filling the tab offers to come back to its share of it, the way Rex's zoom
    // button turns into one.
    let zoom = if zoomed {
        button(&icons::RESTORE, "Restore pane").on_press(move |s: &mut Window| s.tabs.toggle_zoom())
    } else {
        button(&icons::ZOOM, "Zoom pane").on_press(move |s: &mut Window| s.tabs.zoom_at(tab, id))
    };
    let zoom = zoom.build(k).key(format!("block-{id}-zoom"));
    let close = button(&rune_kit::glyphs::CLOSE, "Close pane")
        .on_press(move |s: &mut Window| s.tabs.close_block(tab, id))
        .build(k)
        .key(format!("block-{id}-close"));
    let bar = row()
        .key(format!("block-{id}-header"))
        .w_full()
        .h(HEADER)
        .px(8.0)
        .gap(2.0)
        .center_y();
    bar.child(
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
        // The pane's box glides; the grid inside it takes its final size at once, so a glide
        // reflows the shell once and the pane's rect reveals the new width, rather than the
        // columns chasing the animation at every frame.
        .motion(Spring::NONE)
        .child(screen)
}

/// Whether `node` is, or holds, block `id`.
fn holds(node: &Node, id: u64) -> bool {
    match node {
        Node::Block(block) => *block == id,
        Node::Split { first, second, .. } => holds(first, id) || holds(second, id),
    }
}
