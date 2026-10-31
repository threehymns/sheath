use crate::app::App;
use rune::{El, ScrollSpec, Tx};

// A vertical scroll area with the app's wheel: inertia, overscroll past the ends, and the spring
// back. Rune gives a wheel delta to the viewport under the pointer, so each one takes its own,
// and the content is drawn shifted by the overshoot the scroll holds.
pub(super) fn scrolled(s: &App, key: &'static str, view: El, content: El) -> El {
    let told = s.told(key);
    view.key(key)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .on_wheel(move |s: &mut App, wheel, tx: &mut Tx| s.area_wheel(key, wheel, tx))
        .on_scroll_offset(move |at| told.set(at))
        .child(content.offset(0.0, s.overshoot(key)))
}
