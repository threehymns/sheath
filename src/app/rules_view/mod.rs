mod draft;
mod edit;
mod prop;
mod rules;

use crate::app::App;
use crate::rules::{Kind, Rule};
use kdl::KdlDocument;

pub use draft::RuleEdit;
pub use prop::{RuleMenu, RuleProp};

pub const RULE_ROW_HEIGHT: f32 = 48.0;

// Points a rule press must travel before it counts as a drag rather than a click.
const DRAG_SLOP: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct RuleDrag {
    from: Option<(Kind, usize)>,
    to: usize,
    dy: f32,
    pitch: f32,
}

impl RuleDrag {
    pub fn travelled(dy: f32) -> bool {
        dy.abs() >= DRAG_SLOP
    }

    pub fn dragging(&self) -> Option<(Kind, usize)> {
        self.from
    }

    pub fn moves(&self) -> bool {
        self.from.is_some_and(|(_, from)| from != self.to)
    }

    pub fn follow(&mut self, kind: Kind, index: usize, count: usize, dy: f32, h: f32, gap: f32) {
        if self.from.is_none() {
            if !Self::travelled(dy) {
                return;
            }
            self.from = Some((kind, index));
            self.pitch = (h + gap).max(1.0);
        }
        self.dy = dy;
        let to = index as f32 + (dy / self.pitch).round();
        self.to = to.clamp(0.0, count.saturating_sub(1) as f32) as usize;
    }

    pub fn end(&mut self, cancelled: bool) -> Option<(usize, usize)> {
        let (_, from) = self.from.take()?;
        self.dy = 0.0;
        (!cancelled && from != self.to).then_some((from, self.to))
    }

    pub fn shift(&self, kind: Kind, index: usize) -> f32 {
        let Some((from_kind, from)) = self.from else {
            return 0.0;
        };
        if from_kind != kind {
            return 0.0;
        }
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
    pub(super) fn after_rule_change(&mut self) {
        self.reload_rules();
        self.visit = Default::default();
        self.reload();
    }
}

pub fn rule_key(rules: &[Rule], index: usize) -> Option<String> {
    let rule = rules.get(index)?;
    let summary = rule.summary();
    let occurrence = rules[..index]
        .iter()
        .filter(|other| other.summary() == summary)
        .count();
    Some(format!("rule-{summary}-{occurrence}"))
}

pub(super) fn rule_positions(doc: &KdlDocument) -> Vec<usize> {
    doc.nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node.name().value(), "folder-rule" | "entry-rule"))
        .map(|(at, _)| at)
        .collect()
}

pub(super) fn rule_insertion(doc: &KdlDocument, kind: Kind) -> usize {
    let head = match kind {
        Kind::Folder => "folder-rule",
        Kind::Entry => "entry-rule",
    };
    let positions = rule_positions(doc);
    let after = positions
        .iter()
        .rev()
        .find(|&&at| doc.nodes()[at].name().value() == head)
        .copied()
        .or(positions.last().copied());
    match after {
        Some(at) => at + 1,
        None => doc.nodes().len(),
    }
}
