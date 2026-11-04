//! Dragging a pane by its header: the edge a drop aims at, the badge the frame draws in hand,
//! and the move it makes.
//!
//! A header drag is a gesture on the tab's layout rather than on the block's shell, so the block
//! keeps running while the pointer decides whether it trades places with another pane or takes
//! half of it.

use super::tree::{Node, place, remove, swap};
use super::{Panes, Side};

/// Whether a point falls inside a rect, as a header drop asks of the panes it passes over.
fn inside(rect: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= rect[0] && x <= rect[0] + rect[2] && y >= rect[1] && y <= rect[1] + rect[3]
}

/// How far into a pane's edge a header lands to split beside it rather than swap, as a share of
/// the pane's width or height.
const EDGE_BAND: f32 = 0.25;

/// An edge of a pane a dragged header can be dropped on: the pane it was carried from lands on
/// that side of the pane under the pointer, splitting it in two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// The split that puts the dragged pane on this side of the pane it was dropped on.
    fn split(self) -> (Side, bool) {
        match self {
            Edge::Left => (Side::Right, true),
            Edge::Right => (Side::Right, false),
            Edge::Top => (Side::Down, true),
            Edge::Bottom => (Side::Down, false),
        }
    }

    /// How a drop on this edge reads in the drag badge.
    fn beside(self) -> &'static str {
        match self {
            Edge::Left => "left of",
            Edge::Right => "right of",
            Edge::Top => "above",
            Edge::Bottom => "below",
        }
    }
}

/// Where a dragged header would land: on another pane's body, trading places with it, or on one
/// of its edges, taking half of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Swap(u64),
    Split { pane: u64, edge: Edge },
}

impl DropTarget {
    /// The pane the drop lands on.
    pub(super) fn pane(self) -> u64 {
        match self {
            DropTarget::Swap(pane) | DropTarget::Split { pane, .. } => pane,
        }
    }

    /// What a header let go at `at` means for the pane whose rect it is over: an edge band
    /// splits the pane, and the body swaps the two.
    fn at(rect: [f32; 4], pane: u64, at: (f32, f32)) -> DropTarget {
        let [x, y, w, h] = rect;
        let (w, h) = (w.max(1.0), h.max(1.0));
        let edges = [
            (Edge::Left, (at.0 - x) / w),
            (Edge::Right, (x + w - at.0) / w),
            (Edge::Top, (at.1 - y) / h),
            (Edge::Bottom, (y + h - at.1) / h),
        ];
        match edges.iter().min_by(|a, b| a.1.total_cmp(&b.1)) {
            Some((edge, depth)) if *depth <= EDGE_BAND => DropTarget::Split { pane, edge: *edge },
            _ => DropTarget::Swap(pane),
        }
    }
}

/// What a header drag is carrying and where it would land, for the frame to draw beside the
/// pointer.
pub struct DragPreview {
    pub label: String,
    pub at: (f32, f32),
}

impl Panes {
    /// Where a release would put the header being dragged, for the layout to show.
    pub fn drop_target(&self) -> Option<DropTarget> {
        self.drop
    }

    /// The pane a header drag is carrying, and what letting go would do with it. None until the
    /// pointer moves, so a press alone carries no badge.
    pub fn drag_preview(&self) -> Option<DragPreview> {
        let from = self.dragging?;
        let at = self.drag_at?;
        let name = self.name(from);
        let label = match self.drop {
            Some(DropTarget::Swap(to)) => format!("Swap {name} with {}", self.name(to)),
            Some(DropTarget::Split { pane, edge }) => {
                format!("Move {name} {} {}", edge.beside(), self.name(pane))
            }
            None => format!("Move {name}"),
        };
        Some(DragPreview { label, at })
    }

    /// Picks a block up by its header. A zoomed tab draws one pane and leaves the others with
    /// no rect to aim at, so there is nothing to drop a header on.
    pub fn drag_begin(&mut self, id: u64) {
        if self.zoomed || self.block(id).is_none() {
            return;
        }
        self.dragging = Some(id);
        self.drag_at = None;
        self.drop = None;
    }

    /// The pointer moved over the tab: the pane under it, and the edge it is over, are where a
    /// release would put the block, and the badge follows the pointer there.
    pub fn drag_over(&mut self, at: (f32, f32)) {
        let Some(from) = self.dragging else {
            self.drag_at = None;
            self.drop = None;
            return;
        };
        self.drag_at = Some(at);
        self.drop = self
            .blocks
            .iter()
            .find(|block| block.id != from && inside(block.rect, at))
            .map(|block| DropTarget::at(block.rect, block.id, at));
    }

    /// The header was let go. Over another pane's body the two swap places; over one of its
    /// edges the dragged block takes that side of the pane in a fresh even split. Either way
    /// both shells keep running. Anywhere else, or on a cancelled drag, nothing moves.
    pub fn drag_end(&mut self, cancelled: bool) {
        let Some(from) = self.dragging.take() else {
            self.drag_at = None;
            self.drop = None;
            return;
        };
        self.drag_at = None;
        let Some(drop) = self.drop.take().filter(|_| !cancelled) else {
            return;
        };
        match drop {
            DropTarget::Swap(to) if to != from => swap(&mut self.root, from, to),
            DropTarget::Split { pane, edge } if pane != from => {
                self.move_beside(from, pane, edge);
            }
            _ => {}
        }
    }

    /// Lifts `from` out of the tree and puts it back beside `pane`, on the edge the header was
    /// dropped toward: the split is fresh and even, and every other pane keeps its share.
    fn move_beside(&mut self, from: u64, pane: u64, edge: Edge) {
        let root = std::mem::replace(&mut self.root, Node::Block(0));
        let Some(root) = remove(root, from) else {
            // `from` was the whole tree, so there is no second pane to divide it with.
            self.root = Node::Block(from);
            return;
        };
        self.root = root;
        let (side, before) = edge.split();
        let (first, second) = if before {
            (Node::Block(from), Node::Block(pane))
        } else {
            (Node::Block(pane), Node::Block(from))
        };
        let split = self.next;
        self.next += 1;
        let divided = Node::Split {
            id: split,
            side,
            share: 0.5,
            length: 0.0,
            first: Box::new(first),
            second: Box::new(second),
        };
        place(&mut self.root, pane, divided);
    }
}
