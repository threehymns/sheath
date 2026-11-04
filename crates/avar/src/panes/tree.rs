//! The tree that tiles a tab: blocks at the leaves, splits at the nodes between them.
//!
//! Nothing here knows what a block is beyond its id, or what a split is for beyond the axis it
//! divides along and the share of the space each side takes. The tab holds the tree and asks it
//! for the shares it draws, so a split, a close, a divider drag and a balance are all walks of
//! the same shape.

use super::Side;

/// How a tab's blocks tile: one block, or two layouts divided along one axis. `share` is the
/// part of the split the first side takes, and `length` is the split's extent along that axis,
/// which a drag turns points into a share.
pub(crate) enum Node {
    Block(u64),
    Split {
        id: u64,
        side: Side,
        share: f32,
        length: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}

/// Swaps the block `id` for `divided`, keeping everything around it.
pub(crate) fn place(node: &mut Node, id: u64, divided: Node) -> bool {
    match node {
        Node::Block(block) => {
            if *block == id {
                *node = divided;
                true
            } else {
                false
            }
        }
        Node::Split { first, second, .. } => {
            if contains(first, id) {
                place(first, id, divided)
            } else {
                place(second, id, divided)
            }
        }
    }
}

pub(crate) fn contains(node: &Node, id: u64) -> bool {
    match node {
        Node::Block(block) => *block == id,
        Node::Split { first, second, .. } => contains(first, id) || contains(second, id),
    }
}

/// The splits between the root and a block, outermost first, each with whether the block it
/// leads to is that split's second child.
pub(crate) fn path_to(node: &Node, id: u64, out: &mut Vec<(u64, bool)>) -> bool {
    match node {
        Node::Block(block) => *block == id,
        Node::Split {
            id: split,
            first,
            second,
            ..
        } => {
            if contains(first, id) {
                out.push((*split, false));
                path_to(first, id, out)
            } else if contains(second, id) {
                out.push((*split, true));
                path_to(second, id, out)
            } else {
                false
            }
        }
    }
}

/// Which axis a split divides along, which is what decides whether a resize on that axis can
/// move it.
pub(crate) fn split_side(node: &Node, id: u64) -> Option<Side> {
    match node {
        Node::Block(_) => None,
        Node::Split {
            id: split,
            side,
            first,
            second,
            ..
        } => {
            if *split == id {
                Some(*side)
            } else {
                split_side(first, id).or_else(|| split_side(second, id))
            }
        }
    }
}

/// Every split back to even halves.
pub(crate) fn balance_node(node: &mut Node) {
    if let Node::Split {
        share,
        first,
        second,
        ..
    } = node
    {
        *share = 0.5;
        balance_node(first);
        balance_node(second);
    }
}

/// Takes a block out of the tree, answering what stands in its place: its sibling, or `None` when
/// the block was the whole tree.
pub(crate) fn remove(node: Node, id: u64) -> Option<Node> {
    let Node::Split {
        id: split,
        side,
        share,
        length,
        first,
        second,
    } = node
    else {
        return None;
    };
    if matches!(*first, Node::Block(block) if block == id) {
        return Some(*second);
    }
    if matches!(*second, Node::Block(block) if block == id) {
        return Some(*first);
    }
    if contains(&first, id) {
        let first = remove(*first, id)?;
        Some(Node::Split {
            id: split,
            side,
            share,
            length,
            first: Box::new(first),
            second,
        })
    } else {
        let second = remove(*second, id)?;
        Some(Node::Split {
            id: split,
            side,
            share,
            length,
            first,
            second: Box::new(second),
        })
    }
}

/// A split's share and its extent along the axis, which are the only two numbers a drag writes.
pub(crate) fn split_parts(node: &mut Node, id: u64) -> Option<(&mut f32, &mut f32)> {
    match node {
        Node::Block(_) => None,
        Node::Split {
            id: split,
            share,
            length,
            first,
            second,
            ..
        } => {
            if *split == id {
                Some((share, length))
            } else if let Some(parts) = split_parts(first, id) {
                Some(parts)
            } else {
                split_parts(second, id)
            }
        }
    }
}
