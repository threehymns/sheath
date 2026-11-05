use rune::{El, ScrollSpec, Tx};

use crate::Areas;
use crate::scroll::{Limits, Step};

/// What the crate needs from the host that owns the areas.
pub trait Host {
    fn areas(&self) -> &Areas;
    fn areas_mut(&mut self) -> &mut Areas;
    /// The area's extent, and whether its physics are on. The host reads these from its settings.
    fn scroll_limits(&self, key: &'static str) -> Limits;
    /// Records the viewport's rect. Returns whether anything the view reads moved, so a repeat
    /// of the same rect can skip the rebuild.
    fn record_area(&mut self, key: &'static str, rect: [f32; 4]) -> bool;
}

/// A vertical scroll area with the crate's wheel: inertia, overscroll past the ends, and the
/// spring back. Rune gives a wheel delta to the viewport under the pointer, so each one takes its
/// own, and the content is drawn shifted by the overshoot the scroll holds.
pub fn area<H: Host + 'static>(s: &H, key: &'static str, view: El, content: El) -> El {
    s.areas().mark_shown(key);
    let told = s.areas().offset_cell(key);
    let overshoot = s.areas().overshoot(key);
    view.key(key)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .on_wheel(move |s: &mut H, wheel, tx: &mut Tx| {
            let limits = s.scroll_limits(key);
            match s.areas_mut().wheel(key, &wheel, limits) {
                Some(step) => {
                    // The handler keeps bookkeeping only; a moved viewport paints itself shifted,
                    // so only a step that changed the overshoot rebuilds the view.
                    if !step.redraw {
                        tx.live_only();
                    }
                    apply(key, step, tx);
                    true
                }
                None => false,
            }
        })
        .on_scroll_offset(move |at| told.set(at))
        .on_layout_tx(move |s: &mut H, rect, tx: &mut Tx| {
            // A new size changes what the view builds; a repeat of the same rect can skip it.
            if !s.record_area(key, rect) {
                tx.live_only();
            }
        })
        .child(content.offset(0.0, overshoot))
}

pub(crate) fn apply(key: &'static str, step: Step, tx: &mut Tx) {
    if let Some(at) = step.scroll_to {
        tx.scroll_to_instant(key, 0.0, at);
    }
    if step.again {
        tx.wake_after(std::time::Duration::ZERO);
    }
}
