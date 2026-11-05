//! The motion a tab swap runs on: the effect and the spring the appearance page picks, and the
//! dropdown that is open while it picks them.
//!
//! The choices are read from `settings.kdl` when the window starts and written back as they
//! change, so the frame only has to ask for the transition a swap runs on. A cut leaves the
//! timing with nothing to do, which is why picking one closes the timing dropdown.

use rune::{Transition, Tx};

use crate::Window;
use crate::config;
use crate::settings::{Pick, TabTiming, TabTransition};

impl Window {
    /// How a tab gives way to the next one.
    pub(crate) fn tab_transition(&self) -> TabTransition {
        self.tab_transition
    }

    /// Picks the tab transition, and writes the choice down so the next window opens the same
    /// way. A cut leaves the timing with nothing to do, so its dropdown closes with it.
    pub(crate) fn set_tab_transition(&mut self, transition: TabTransition) {
        self.tab_transition = transition;
        if transition == TabTransition::Cut {
            self.pick = None;
        }
        self.prefs.set_choice(
            config::TAB_TRANSITION,
            transition.name(),
            TabTransition::default().name(),
        );
    }

    /// The spring the tab transition runs on.
    pub(crate) fn tab_timing(&self) -> TabTiming {
        self.tab_timing
    }

    /// Picks the transition's spring, and writes the choice down with the rest of the settings.
    pub(crate) fn set_tab_timing(&mut self, timing: TabTiming) {
        self.tab_timing = timing;
        self.prefs.set_choice(
            config::TAB_TIMING,
            timing.name(),
            TabTiming::default().name(),
        );
    }

    /// Marks the commit a tab switch runs in. A cut has no motion to play, so the swap and the
    /// chrome the window rebuilds around the new tab (a lone shell's tab has none, a split one
    /// has a frame, a toolbar and dividers) land in the frame it happened. A transition leaves
    /// that layout to the springs the appearance page picked.
    pub(crate) fn tab_switched(&self, tx: &mut Tx) {
        if self.tab_transition == TabTransition::Cut {
            tx.instant();
        }
    }

    /// The presence transition a tab swap runs on: the chosen effect with the chosen spring, or
    /// no motion at all when the effect is a cut.
    pub(crate) fn tab_swap(&self) -> Transition {
        let effect = self.tab_transition.effect();
        if self.tab_transition == TabTransition::Cut {
            effect
        } else {
            effect.springs(self.tab_timing.springs())
        }
    }

    /// The appearance page's open dropdown, if one is.
    pub(crate) fn pick(&self) -> Option<Pick> {
        self.pick
    }

    /// Opens a dropdown, or closes it when it is the one already open.
    pub(crate) fn toggle_pick(&mut self, pick: Pick) {
        self.pick = (self.pick != Some(pick)).then_some(pick);
    }

    pub(crate) fn clear_pick(&mut self) {
        self.pick = None;
    }
}
