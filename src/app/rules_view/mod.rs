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
