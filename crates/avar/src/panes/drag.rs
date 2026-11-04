//! Dragging a pane by its header: the edge a drop aims at, the layout it would leave, and the
//! move it makes.
//!
//! The committed tree stays put while the pointer is down: the tab draws a preview of the drop
//! instead, and only a release writes the change. A cancel drops the preview, so the layout on
//! screen is the one the user had before they picked the header up.

use super::tree::{Node, place, remove, swap};
use super::{Panes, Side};

/// Whether a point falls inside a rect, as a header drop asks of the panes it passes over.
fn inside(rect: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= rect[0] && x <= rect[0] + rect[2] && y >= rect[1] && y <= rect[1] + rect[3]
}

/// How far into a pane's edge a header lands to split beside it rather than swap, as a share of
/// the pane's width or height.
const EDGE_BAND: f32 = 0.25;

/// The split a preview draws before the drop is committed. It never enters the tab's tree, so it
/// only has to be a name the layout can key and the shares can ignore.
const PREVIEW_SPLIT: u64 = u64::MAX;

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

impl Panes {
    /// Where a release would put the header being dragged, for the layout to show.
    pub fn drop_target(&self) -> Option<DropTarget> {
        self.drop
    }

    /// Picks a block up by its header. `at` is the press in the window, which the moves measure
    /// their travel from: the pane it came from may glide away under the preview, so the pointer
    /// must not be read back from that pane's rect. A zoomed tab draws one pane and leaves the
    /// others with no rect to aim at, so there is nothing to drop a header on.
    pub fn drag_begin(&mut self, id: u64, at: (f32, f32)) {
        if self.zoomed || self.block(id).is_none() {
            return;
        }
        self.dragging = Some(id);
        self.press = Some(at);
        self.drop = None;
        self.preview = None;
    }

    /// The pointer travelled `travel` points from the press: the pane under it, and the edge it
    /// is over, are where a release would put the block, and the tab draws that layout now.
    pub fn drag_over(&mut self, travel: (f32, f32)) {
        let Some(from) = self.dragging else {
            self.press = None;
            self.drop = None;
            self.preview = None;
            return;
        };
        let Some(press) = self.press else {
            return;
        };
        let at = (press.0 + travel.0, press.1 + travel.1);
        self.drop = self
            .blocks
            .iter()
            .find(|block| block.id != from && inside(block.rect, at))
            .map(|block| DropTarget::at(block.rect, block.id, at));
        self.preview = self.drop.and_then(|drop| self.preview_of(from, drop));
    }

    /// The header was let go. Over another pane's body the two swap places; over one of its
    /// edges the dragged block takes that side of the pane in a fresh even split. Either way
    /// both shells keep running. Anywhere else, or on a cancelled drag, the preview goes and
    /// nothing moves.
    pub fn drag_end(&mut self, cancelled: bool) {
        let Some(from) = self.dragging.take() else {
            self.press = None;
            self.drop = None;
            self.preview = None;
            return;
        };
        self.press = None;
        self.preview = None;
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

    /// The layout this drop would leave: the committed tree with the block moved or swapped,
    /// which the tab draws in place of the real one until the release.
    fn preview_of(&self, from: u64, drop: DropTarget) -> Option<Node> {
        match drop {
            DropTarget::Swap(to) if to != from => {
                let mut root = self.root.clone();
                swap(&mut root, from, to);
                Some(root)
            }
            DropTarget::Split { pane, edge } if pane != from => {
                beside(self.root.clone(), from, pane, edge, PREVIEW_SPLIT)
            }
            _ => None,
        }
    }

    /// Lifts `from` out of the tree and puts it back beside `pane`, on the edge the header was
    /// dropped toward: the split is fresh and even, and every other pane keeps its share.
    fn move_beside(&mut self, from: u64, pane: u64, edge: Edge) {
        let split = self.next;
        self.next += 1;
        let root = std::mem::replace(&mut self.root, Node::Block(0));
        self.root = beside(root, from, pane, edge, split).unwrap_or(Node::Block(from));
    }
}

/// `root` with `from` lifted out and put beside `pane` on `edge`, in a split called `split`.
/// None when `from` was the whole tree, leaving nothing to divide.
fn beside(mut root: Node, from: u64, pane: u64, edge: Edge, split: u64) -> Option<Node> {
    root = remove(root, from)?;
    let (side, before) = edge.split();
    let (first, second) = if before {
        (Node::Block(from), Node::Block(pane))
    } else {
        (Node::Block(pane), Node::Block(from))
    };
    let divided = Node::Split {
        id: split,
        side,
        share: 0.5,
        length: 0.0,
        first: Box::new(first),
        second: Box::new(second),
    };
    place(&mut root, pane, divided);
    Some(root)
}
