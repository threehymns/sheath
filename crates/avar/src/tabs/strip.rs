//! The bar across the top of the tab on screen: one chip per tab, the active one filled, and a
//! drag that reorders them across the bar.

use rune::{Anim, Drag, El, Phase, Tx, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit, glyphs};

use super::Tabs;
use crate::Window;

/// The bar's height when the tabs are horizontal, and the chip that sits in it.
const STRIP: f32 = 36.0;
const CHIP: f32 = 26.0;
/// Points between chips in the bar, and the pointer travel that turns a press on one of them into
/// a drag to reorder, as the rail's rows have.
const BAR_GAP: f32 = 2.0;
const SLOP: f32 = 4.0;

/// The bar's drag to reorder, measured across. The kit's [`Reorder`] is the rail's: it moves rows
/// down a list, where they share one height. The bar keeps the same gesture for chips that share
/// a row but not a width, so each chip reports the width it was laid out at and the offsets come
/// from those.
#[derive(Default)]
pub(super) struct Strip {
    from: Option<usize>,
    to: usize,
    dx: f32,
    /// Each chip's width, by its place in the bar, as the last layout reported it.
    widths: Vec<f32>,
}

impl Strip {
    /// The chip being dragged, by its index before the drag.
    fn dragging(&self) -> Option<usize> {
        self.from
    }

    /// The pointer moved `dx` since the press on chip `index`, in a bar of `count` chips.
    fn follow(&mut self, index: usize, count: usize, dx: f32) {
        if self.from.is_none() {
            if dx.abs() < SLOP {
                return;
            }
            self.from = Some(index);
        }
        self.dx = dx;
        self.to = self.target(index, count);
    }

    /// The drag ended: `(from, to)` for a drop that moves a chip, `None` for a cancel or a drop
    /// in place.
    fn end(&mut self, cancelled: bool) -> Option<(usize, usize)> {
        let from = self.from.take()?;
        self.dx = 0.0;
        (!cancelled && from != self.to).then_some((from, self.to))
    }

    /// Where the chip in hand would land: the last chip whose middle it has passed, within the
    /// bar's ends.
    fn target(&self, from: usize, count: usize) -> usize {
        if count == 0 {
            return from;
        }
        let middle = self.middle(from) + self.dx;
        let mut to = from;
        while to + 1 < count && middle > self.middle(to + 1) {
            to += 1;
        }
        while to > 0 && middle < self.middle(to - 1) {
            to -= 1;
        }
        to
    }

    /// How far chip `index` sits from its place: the chip in hand with the pointer, and the chips
    /// between its place and its target a stride toward the gap it left.
    fn shift(&self, index: usize) -> f32 {
        let Some(from) = self.from else { return 0.0 };
        if index == from {
            return self.dx;
        }
        // A chip makes room by the one in hand's width, which is the space it gives up.
        let stride = self.width_of(from) + BAR_GAP;
        if from < index && index <= self.to {
            -stride
        } else if self.to <= index && index < from {
            stride
        } else {
            0.0
        }
    }

    /// Chip `index`'s left edge in the bar, from the widths alone: the padding before the first
    /// chip is the same for all of them, so it cancels out of every offset.
    fn base(&self, index: usize) -> f32 {
        (0..index).map(|i| self.width_of(i) + BAR_GAP).sum()
    }

    fn middle(&self, index: usize) -> f32 {
        self.base(index) + self.width_of(index) / 2.0
    }

    fn width_of(&self, index: usize) -> f32 {
        self.widths.get(index).copied().unwrap_or(0.0)
    }

    /// A chip with the bar's drag on it: the offset that makes room for the chip in hand, the
    /// lift while it is held, and the handlers that follow the pointer and drop it.
    fn wire(&self, chip: El, index: usize, k: &Kit) -> El {
        let held = self.dragging() == Some(index);
        let mut chip = chip.offset(self.shift(index), 0.0);
        if held {
            // Above its neighbours and opaque, so the chips it passes don't show through it, and
            // following the pointer exactly while the rest of the bar springs around it.
            chip = chip.raise().offset_motion(Anim::INSTANT).fill(k.c.layer_02);
        }
        chip.on_drag(
            move |s: &mut Window, d: &mut Drag, tx: &mut Tx| match d.phase {
                Phase::Down => {}
                Phase::Move => {
                    tx.animated();
                    let count = s.tabs.rows.len();
                    s.tabs.strip.follow(index, count, d.dx);
                }
                Phase::Up => {
                    if let Some((from, to)) = s.tabs.strip.end(d.cancelled()) {
                        s.tabs.move_row(from, to);
                    }
                }
            },
        )
        .on_layout_tx(move |s: &mut Window, rect, tx: &mut Tx| {
            let widths = &mut s.tabs.strip.widths;
            if widths.len() <= index {
                widths.resize(index + 1, 0.0);
            }
            widths[index] = rect[2];
            // The widths steer the next drag; nothing in the view is built from them.
            tx.live_only();
        })
    }
}

impl Tabs {
    /// The tabs as a bar across the top of the tab on screen, for the horizontal layout in the
    /// settings: one chip per tab, the active one filled, and its close button on the chip. A
    /// chip drags across the bar to reorder the tabs, as a row drags down the rail.
    pub fn strip(&self, k: &Kit) -> El {
        let mut bar = row()
            .key("tab-strip")
            .role("tablist")
            .w_full()
            .h(STRIP)
            .px(6.0)
            .gap(BAR_GAP)
            .center_y()
            .transition(rune::Transition::none());
        for (index, tab) in self.rows.iter().enumerate() {
            let id = tab.id;
            let selected = index == self.active;
            let blocks = tab.panes.blocks();
            let title = tab.panes.title();
            let mut chip = row()
                .key(format!("tab-{id}"))
                .role("tab")
                .label(title.clone())
                .h(CHIP)
                .min_w(0.0)
                .max_w(200.0)
                .px(8.0)
                .gap(6.0)
                .center_y()
                .radius(k.radius.sm)
                .motion(k.motion.control)
                .child(
                    text(title)
                        .key("label")
                        .font(k.ty.small)
                        .color(if selected {
                            k.c.text_base
                        } else {
                            k.c.text_muted
                        })
                        .truncate()
                        .grow(1.0)
                        .shrink(),
                );
            // A split tab says how many blocks it holds, as its row in the rail does.
            if blocks > 1 {
                chip = chip.child(
                    text(format!("{blocks}"))
                        .key("count")
                        .font(k.ty.small)
                        .color(k.c.text_faint),
                );
            }
            if selected {
                chip = chip.fill(k.c.pressed);
            }
            bar = bar.child(
                self.strip.wire(
                    chip.child(
                        IconButton::new(&glyphs::CLOSE)
                            .variant(IconButtonVariant::GhostMuted)
                            .control_size(ControlSize::Mini)
                            .label("Close tab")
                            .help("Close tab")
                            .on_press(move |s: &mut Window| s.tabs.close(id))
                            .build(k)
                            .key(format!("tab-{id}-close")),
                    )
                    .hover(k.c.hover)
                    .on_press(move |s: &mut Window| s.tabs.select(id)),
                    index,
                    k,
                ),
            );
        }
        bar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bar of three chips, as the layout would have measured them.
    fn bar() -> Strip {
        Strip {
            widths: vec![100.0, 80.0, 120.0],
            ..Strip::default()
        }
    }

    #[test]
    fn a_press_inside_the_slop_is_still_a_press() {
        let mut strip = bar();
        strip.follow(0, 3, SLOP - 1.0);
        assert_eq!(strip.dragging(), None);
        assert_eq!(strip.shift(0), 0.0);
        assert_eq!(
            strip.end(false),
            None,
            "a press that doesn't move drops nothing"
        );
    }

    #[test]
    fn the_chips_passed_step_aside_by_the_one_in_hands_width() {
        let mut strip = bar();
        // The middles are 50, 142 and 244; the chip in hand is dragged to 150, past the second.
        strip.follow(0, 3, 100.0);
        assert_eq!(strip.dragging(), Some(0));
        assert_eq!(strip.to, 1);
        assert_eq!(strip.shift(0), 100.0, "it follows the pointer exactly");
        assert_eq!(
            strip.shift(1),
            -102.0,
            "the chip it passed gives up its width"
        );
        assert_eq!(strip.shift(2), 0.0);
        assert_eq!(strip.end(false), Some((0, 1)));
        assert_eq!(strip.shift(1), 0.0);
    }

    #[test]
    fn a_drag_clamps_to_the_bars_ends_and_a_cancel_moves_nothing() {
        let mut strip = bar();
        strip.follow(2, 3, -1000.0);
        assert_eq!(strip.dragging(), Some(2));
        assert_eq!(strip.to, 0);
        assert_eq!(strip.shift(1), 122.0, "the chips it passed make room");
        assert_eq!(strip.end(true), None);
    }
}
