//! The Rules tab's work: the form that holds a rule as a draft until it is saved, and the
//! one-shot actions that write a rule at once. Every change goes through the config store, so
//! the rest of the file keeps its comments and layout.
//!
//! `edit` owns the form, its draft, and what the file can accept; `rules` owns saving the view,
//! pinning an entry, deleting, reordering, and the picker.

mod edit;
mod rules;

use crate::app::App;
use kdl::KdlDocument;

pub use edit::{RuleEdit, RuleProp};

/// The height of one rule's row. The drag handler turns a pointer's travel into rows with it.
pub const RULE_ROW_HEIGHT: f32 = 56.0;

/// Points the pointer travels from a press before it means a drag rather than a click.
const DRAG_SLOP: f32 = 4.0;

/// A rule list's drag to a new place: the row in hand, where it would land, and how far it sits
/// from its place. This is Rune's own reorder model, `rune_kit::shell::Reorder`, which the kit
/// only wires up to its sidebar rows; the tab carries its own copy so a rule row keeps its
/// shape. A row in hand lifts and follows the pointer, and the rows it passes step aside, so
/// the gap it would drop into is the order it would land in.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuleDrag {
    /// The row in hand, once the pointer has travelled far enough to mean a move.
    from: Option<usize>,
    /// Where that row would land if it were dropped now.
    to: usize,
    /// How far the pointer has travelled since the press.
    dy: f32,
    /// A row's height plus the hairline under it: what one step of the list moves.
    pitch: f32,
}

impl RuleDrag {
    /// Whether a pointer that has travelled `dy` from a press is dragging rather than clicking.
    pub fn travelled(dy: f32) -> bool {
        dy.abs() >= DRAG_SLOP
    }

    /// The row being dragged, once the gesture has outgrown a click.
    pub fn dragging(&self) -> Option<usize> {
        self.from
    }

    /// The pointer moved `dy` since the press on row `index`, which is `h` tall; `gap` is the
    /// hairline between two rows, which one step carries the rows across too.
    pub fn follow(&mut self, index: usize, count: usize, dy: f32, h: f32, gap: f32) {
        if self.from.is_none() {
            if !Self::travelled(dy) {
                return;
            }
            self.from = Some(index);
            self.pitch = (h + gap).max(1.0);
        }
        self.dy = dy;
        let to = index as f32 + (dy / self.pitch).round();
        self.to = to.clamp(0.0, count.saturating_sub(1) as f32) as usize;
    }

    /// The gesture ended: `(from, to)` for a drop that reorders, `None` for a cancel, a drop in
    /// place, or a press that never moved.
    pub fn end(&mut self, cancelled: bool) -> Option<(usize, usize)> {
        let from = self.from.take()?;
        self.dy = 0.0;
        (!cancelled && from != self.to).then_some((from, self.to))
    }

    /// How far row `index` sits from its place: the row in hand follows the pointer exactly, and
    /// the rows between its place and its target step toward the gap it left.
    pub fn shift(&self, index: usize) -> f32 {
        let Some(from) = self.from else {
            return 0.0;
        };
        if index == from {
            self.dy
        } else if from < index && index <= self.to {
            -self.pitch
        } else if self.to <= index && index < from {
            self.pitch
        } else {
            0.0
        }
    }
}

impl App {
    /// Re-reads the rules and the folder after the document changed.
    pub(super) fn after_rule_change(&mut self) {
        self.reload_rules();
        self.visit = Default::default();
        self.reload();
    }
}

/// The document positions of the rule nodes, in file order.
pub(super) fn rule_positions(doc: &KdlDocument) -> Vec<usize> {
    doc.nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node.name().value(), "folder-rule" | "entry-rule"))
        .map(|(at, _)| at)
        .collect()
}
