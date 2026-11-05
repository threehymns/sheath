//! The motion a tab swap runs on: the effect and the spring the appearance page picks, and the
//! dropdown that is open while it picks them.
//!
//! The choices are read from `settings.kdl` when the window starts and written back as they
//! change, so the frame only has to ask for the transition a swap runs on. A cut leaves the
//! timing with nothing to do, which is why picking one closes the timing dropdown.

use rune::{Edge, Effect, SpringPair, Transition, Tx, Vec2};

use crate::Window;
use crate::config;
use crate::settings::Pick;

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

    /// Which edge a slide between tabs comes in from.
    pub(crate) fn tab_direction(&self) -> TabDirection {
        self.tab_direction
    }

    /// Picks the slide's direction, and writes the choice down with the rest of the settings.
    pub(crate) fn set_tab_direction(&mut self, direction: TabDirection) {
        self.tab_direction = direction;
        self.prefs.set_choice(
            config::TAB_DIRECTION,
            direction.name(),
            TabDirection::default().name(),
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
        let effect = self
            .tab_transition
            .effect(self.tab_direction, self.tabs.switched_forward());
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

/// How the tab on screen gives way to the next one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TabTransition {
    /// No motion: the next tab is simply there.
    #[default]
    Cut,
    /// The next tab fades in as the last one fades out.
    Fade,
    /// The next tab slides in from the leading edge while the last one leaves through the
    /// trailing one.
    SlideHorizontal,
    /// The same along the vertical axis: in from the top, out through the bottom.
    SlideVertical,
}

impl TabTransition {
    pub(crate) const ALL: [TabTransition; 4] = [
        TabTransition::Cut,
        TabTransition::Fade,
        TabTransition::SlideHorizontal,
        TabTransition::SlideVertical,
    ];

    /// The name `settings.kdl` writes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            TabTransition::Cut => "cut",
            TabTransition::Fade => "fade",
            TabTransition::SlideHorizontal => "slide-horizontal",
            TabTransition::SlideVertical => "slide-vertical",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            TabTransition::Cut => "Cut",
            TabTransition::Fade => "Fade",
            TabTransition::SlideHorizontal => "Slide horizontal",
            TabTransition::SlideVertical => "Slide vertical",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<TabTransition> {
        TabTransition::ALL
            .into_iter()
            .find(|transition| transition.name() == name)
    }

    pub(crate) fn from_label(label: &str) -> Option<TabTransition> {
        TabTransition::ALL
            .into_iter()
            .find(|transition| transition.label() == label)
    }

    /// The presence effect alone; the window layers the chosen timing on top. `forward` says
    /// whether the tab being shown sits further along the rail than the one leaving.
    pub(crate) fn effect(self, direction: TabDirection, forward: bool) -> Transition {
        // Following the tab order, a tab to the right slides in from the right; fixed, every
        // slide comes in from the leading edge, as it always did.
        let forward = direction == TabDirection::Follow && forward;
        match self {
            TabTransition::Cut => Transition::none(),
            TabTransition::Fade => Transition::opacity(),
            TabTransition::SlideHorizontal if forward => Transition::push(Edge::Trailing),
            TabTransition::SlideHorizontal => Transition::slide(),
            TabTransition::SlideVertical => {
                let (enter, leave) = if forward { (1.0, -1.0) } else { (-1.0, 1.0) };
                let offset = |y: f32| Effect {
                    offset_fraction: Vec2::new(0.0, y),
                    ..Effect::IDENTITY
                };
                Transition {
                    insertion: offset(enter),
                    removal: offset(leave),
                    ..Transition::slide()
                }
            }
        }
    }
}

/// Which edge a slide between tabs comes in from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TabDirection {
    /// The side the new tab sits on: the next tab comes in from the far side, the last one
    /// from the near side, so a walk through the tabs reads as one.
    #[default]
    Follow,
    /// The leading edge, or the top on the vertical axis, whatever the tab's place.
    Fixed,
}

impl TabDirection {
    pub(crate) const ALL: [TabDirection; 2] = [TabDirection::Follow, TabDirection::Fixed];

    /// The name `settings.kdl` writes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            TabDirection::Follow => "follow",
            TabDirection::Fixed => "fixed",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            TabDirection::Follow => "Follow the tab order",
            TabDirection::Fixed => "One way",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<TabDirection> {
        TabDirection::ALL
            .into_iter()
            .find(|direction| direction.name() == name)
    }

    pub(crate) fn from_label(label: &str) -> Option<TabDirection> {
        TabDirection::ALL
            .into_iter()
            .find(|direction| direction.label() == label)
    }
}

/// The spring a tab transition runs on, named after the motion tokens Rune already ships: from
/// barely there through the springy presets that overshoot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TabTiming {
    Instant,
    Snap,
    #[default]
    Settle,
    Reflow,
    Glide,
    Morph,
    Pop,
    Drift,
}

impl TabTiming {
    pub(crate) const ALL: [TabTiming; 8] = [
        TabTiming::Instant,
        TabTiming::Snap,
        TabTiming::Settle,
        TabTiming::Reflow,
        TabTiming::Glide,
        TabTiming::Morph,
        TabTiming::Pop,
        TabTiming::Drift,
    ];

    /// The name `settings.kdl` writes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            TabTiming::Instant => "instant",
            TabTiming::Snap => "snap",
            TabTiming::Settle => "settle",
            TabTiming::Reflow => "reflow",
            TabTiming::Glide => "glide",
            TabTiming::Morph => "morph",
            TabTiming::Pop => "pop",
            TabTiming::Drift => "drift",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            TabTiming::Instant => "Instant",
            TabTiming::Snap => "Snap",
            TabTiming::Settle => "Settle",
            TabTiming::Reflow => "Reflow",
            TabTiming::Glide => "Glide",
            TabTiming::Morph => "Morph",
            TabTiming::Pop => "Pop",
            TabTiming::Drift => "Drift",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<TabTiming> {
        TabTiming::ALL
            .into_iter()
            .find(|timing| timing.name() == name)
    }

    pub(crate) fn from_label(label: &str) -> Option<TabTiming> {
        TabTiming::ALL
            .into_iter()
            .find(|timing| timing.label() == label)
    }

    pub(crate) fn springs(self) -> SpringPair {
        use rune::tok::motion::pair;
        match self {
            TabTiming::Instant => pair::INSTANT,
            TabTiming::Snap => pair::SNAP,
            TabTiming::Settle => pair::SETTLE,
            TabTiming::Reflow => pair::REFLOW,
            TabTiming::Glide => pair::GLIDE,
            TabTiming::Morph => pair::MORPH,
            TabTiming::Pop => pair::POP,
            TabTiming::Drift => pair::DRIFT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slide follows the tab order: the tab after the one leaving sits to its right (below it
    /// on the vertical axis) and comes in from there, and the tab before it from the other side.
    /// One direction keeps the leading edge, whatever the tab's place.
    #[test]
    fn a_following_slide_enters_from_the_side_the_tab_sits_on() {
        let enter = |transition: TabTransition, direction: TabDirection, forward: bool| {
            let effect = transition.effect(direction, forward).insertion;
            (effect.offset_fraction.x, effect.offset_fraction.y)
        };
        let (h, v) = (TabTransition::SlideHorizontal, TabTransition::SlideVertical);
        assert_eq!(
            enter(h, TabDirection::Follow, true),
            (1.0, 0.0),
            "the next tab comes in from the right"
        );
        assert_eq!(
            enter(h, TabDirection::Follow, false),
            (-1.0, 0.0),
            "the last one comes in from the left"
        );
        assert_eq!(
            enter(v, TabDirection::Follow, true),
            (0.0, 1.0),
            "the next tab comes in from below"
        );
        assert_eq!(
            enter(v, TabDirection::Follow, false),
            (0.0, -1.0),
            "the last one comes in from above"
        );
        for transition in [h, v] {
            assert_eq!(
                enter(transition, TabDirection::Fixed, true),
                enter(transition, TabDirection::Fixed, false),
                "one direction slides the same way either way"
            );
        }
    }
}
