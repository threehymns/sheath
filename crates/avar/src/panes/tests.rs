use super::*;

/// A tab in the temporary folder with a block and no shell started: opening a terminal only
/// queues the shell for the first poll, and a test never polls.
fn tab() -> Panes {
    Panes::new(std::env::temp_dir())
}

/// Every split's share, outermost first.
fn shares(node: &Node) -> Vec<f32> {
    match node {
        Node::Block(_) => Vec::new(),
        Node::Split {
            share,
            first,
            second,
            ..
        } => {
            let mut out = vec![*share];
            out.extend(shares(first));
            out.extend(shares(second));
            out
        }
    }
}

/// The direction each split divides along, outermost first.
fn sides(node: &Node) -> Vec<Side> {
    match node {
        Node::Block(_) => Vec::new(),
        Node::Split {
            side,
            first,
            second,
            ..
        } => {
            let mut out = vec![*side];
            out.extend(sides(first));
            out.extend(sides(second));
            out
        }
    }
}

/// The blocks in the tree, left-to-right and top-to-bottom.
fn order(node: &Node) -> Vec<u64> {
    match node {
        Node::Block(id) => vec![*id],
        Node::Split { first, second, .. } => {
            let mut out = order(first);
            out.extend(order(second));
            out
        }
    }
}

#[test]
fn a_split_puts_the_new_block_beside_the_one_it_divided() {
    let mut p = tab();
    let first = p.focus_id();
    p.split(Side::Right);
    assert_eq!(p.blocks(), 2);
    assert_eq!(shares(&p.root), vec![0.5], "a new split is even");
    assert_ne!(p.focus_id(), first, "the new block takes the keyboard");
    assert!(
        p.block(first).is_some(),
        "the divided block keeps its shell"
    );
}

#[test]
fn resize_grows_the_focused_block_toward_the_direction() {
    let mut p = tab();
    p.split(Side::Right);
    let first = 1;
    // The block on the right takes from the one on its left.
    p.resize(Dir::Left, 10.0);
    assert!(
        (shares(&p.root)[0] - 0.4).abs() < 1e-4,
        "{:?}",
        shares(&p.root)
    );
    // It reaches the tab's right edge, so there is no boundary to push that way.
    p.resize(Dir::Right, 10.0);
    assert!(
        (shares(&p.root)[0] - 0.4).abs() < 1e-4,
        "{:?}",
        shares(&p.root)
    );
    p.focus(first);
    p.resize(Dir::Right, 15.0);
    assert!(
        (shares(&p.root)[0] - 0.55).abs() < 1e-4,
        "{:?}",
        shares(&p.root)
    );
}

#[test]
fn dropping_a_header_on_a_pane_body_swaps_the_two() {
    let mut p = tab();
    p.split(Side::Right);
    let left = 1;
    let right = p.focus_id();
    p.set_rect(left, [0.0, 0.0, 100.0, 100.0]);
    p.set_rect(right, [100.0, 0.0, 100.0, 100.0]);

    p.drag_begin(left);
    p.drag_over((150.0, 50.0));
    assert_eq!(
        p.drop_target(),
        Some(DropTarget::Swap(right)),
        "the pane under the header"
    );
    p.drag_end(false);

    assert_eq!(
        order(&p.root),
        vec![right, left],
        "the shells traded places"
    );
    assert_eq!(p.focus_id(), right, "the keyboard stays where it was");
}

#[test]
fn dropping_a_header_on_a_pane_edge_splits_beside_it() {
    let mut p = tab();
    p.split(Side::Right);
    let left = 1;
    let right = p.focus_id();
    p.set_rect(left, [0.0, 0.0, 100.0, 100.0]);
    p.set_rect(right, [100.0, 0.0, 100.0, 100.0]);

    // Near the target's top edge: the drag lands above it rather than swapping.
    p.drag_begin(left);
    p.drag_over((150.0, 5.0));
    assert_eq!(
        p.drop_target(),
        Some(DropTarget::Split {
            pane: right,
            edge: Edge::Top
        })
    );
    p.drag_end(false);

    assert_eq!(order(&p.root), vec![left, right]);
    assert_eq!(sides(&p.root), vec![Side::Down], "the pane sits above");
    assert_eq!(p.blocks(), 2, "the moved block is not duplicated");

    // The block in the second half of a split comes out too: dropping it on the other pane's
    // left edge opens a fresh row.
    p.drag_begin(right);
    p.drag_over((5.0, 50.0));
    p.drag_end(false);

    assert_eq!(order(&p.root), vec![right, left]);
    assert_eq!(sides(&p.root), vec![Side::Right], "the pane sits left");
    assert_eq!(p.blocks(), 2);
}

#[test]
fn a_header_dropped_off_any_pane_or_cancelled_moves_nothing() {
    let mut p = tab();
    p.split(Side::Right);
    let left = 1;
    let right = p.focus_id();
    p.set_rect(left, [0.0, 0.0, 100.0, 100.0]);
    p.set_rect(right, [100.0, 0.0, 100.0, 100.0]);

    p.drag_begin(left);
    p.drag_over((500.0, 500.0));
    assert_eq!(p.drop_target(), None);
    p.drag_end(false);
    assert_eq!(order(&p.root), vec![left, right]);

    p.drag_begin(left);
    p.drag_over((150.0, 50.0));
    p.drag_end(true);
    assert_eq!(order(&p.root), vec![left, right], "a cancelled drag holds");
}

#[test]
fn a_resize_stops_at_the_smallest_share() {
    let mut p = tab();
    p.split(Side::Right);
    for _ in 0..40 {
        p.resize(Dir::Left, 5.0);
    }
    assert!((shares(&p.root)[0] - MIN_SHARE).abs() < 1e-4);
}

#[test]
fn balance_evens_every_split_and_a_closed_block_leaves_its_sibling_the_space() {
    let mut p = tab();
    p.split(Side::Right);
    p.split(Side::Down);
    assert_eq!(p.blocks(), 3);
    p.resize(Dir::Up, 20.0);
    p.balance();
    assert!(shares(&p.root).iter().all(|s| (*s - 0.5).abs() < 1e-4));
    let closing = p.focus_id();
    p.close(closing);
    assert_eq!(p.blocks(), 2);
    assert!(p.block(closing).is_none());
    assert!(
        p.block(p.focus_id()).is_some(),
        "the keyboard lands on a live block"
    );
}
