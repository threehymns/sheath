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

/// A tab split left and right, with both panes laid out so a drag has rects to aim at.
fn pair() -> (Panes, u64, u64) {
    let mut p = tab();
    p.split(Side::Right);
    let left = 1;
    let right = p.focus_id();
    p.set_rect(left, [0.0, 0.0, 100.0, 100.0]);
    p.set_rect(right, [100.0, 0.0, 100.0, 100.0]);
    (p, left, right)
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
    let (mut p, left, right) = pair();

    p.drag_begin((10.0, 10.0));
    p.drag_over((140.0, 40.0));
    assert_eq!(
        p.drop,
        Some(DropTarget::Swap(right)),
        "the pane under the header"
    );
    assert_eq!(
        order(p.preview.as_ref().expect("the swap is previewed")),
        vec![right, left],
        "the tab shows the shells already traded"
    );
    assert_eq!(
        order(&p.root),
        vec![left, right],
        "the tab waits for the drop"
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
    let (mut p, left, right) = pair();

    // Near the target's top edge: the drag lands above it rather than swapping.
    p.drag_begin((10.0, 10.0));
    p.drag_over((140.0, -5.0));
    assert_eq!(
        p.drop,
        Some(DropTarget::Split {
            pane: right,
            edge: Edge::Top
        })
    );
    assert_eq!(
        sides(p.preview.as_ref().expect("the split is previewed")),
        vec![Side::Down],
        "the preview is already a column"
    );
    assert_eq!(
        sides(&p.root),
        vec![Side::Right],
        "the tab waits for the drop"
    );
    p.drag_end(false);

    assert_eq!(order(&p.root), vec![left, right]);
    assert_eq!(sides(&p.root), vec![Side::Down], "the pane sits above");
    assert_eq!(p.blocks(), 2, "the moved block is not duplicated");

    // The block in the second half of a split comes out too: dropping it on the other pane's
    // left edge opens a fresh row.
    p.drag_begin((110.0, 10.0));
    p.drag_over((-105.0, 40.0));
    p.drag_end(false);

    assert_eq!(order(&p.root), vec![right, left]);
    assert_eq!(sides(&p.root), vec![Side::Right], "the pane sits left");
    assert_eq!(p.blocks(), 2);
}

#[test]
fn a_drag_previews_the_layout_and_a_cancel_puts_it_back() {
    let (mut p, left, right) = pair();

    p.drag_begin((10.0, 10.0));
    assert!(p.preview.is_none(), "a press alone rearranges nothing");

    p.drag_over((140.0, 40.0));
    assert_eq!(
        order(p.preview.as_ref().expect("the swap is previewed")),
        vec![right, left]
    );
    // The panes move under the pointer, so the drag keeps aiming at the layout it started in
    // instead of chasing the preview.
    p.set_rect(left, [900.0, 900.0, 1.0, 1.0]);
    assert_eq!(p.block(left).unwrap().rect, [0.0, 0.0, 100.0, 100.0]);

    p.drag_end(true);
    assert!(p.preview.is_none(), "a cancel drops the preview");
    assert_eq!(order(&p.root), vec![left, right], "the layout holds");
}

#[test]
fn a_header_dropped_off_any_pane_or_cancelled_moves_nothing() {
    let (mut p, left, right) = pair();

    p.drag_begin((10.0, 10.0));
    p.drag_over((490.0, 490.0));
    assert_eq!(p.drop, None);
    assert!(p.preview.is_none(), "no pane means no preview");
    p.drag_end(false);
    assert_eq!(order(&p.root), vec![left, right]);

    p.drag_begin((10.0, 10.0));
    p.drag_over((140.0, 40.0));
    p.drag_end(true);
    assert_eq!(order(&p.root), vec![left, right], "a cancelled drag holds");
}

#[test]
fn only_a_press_in_a_header_picks_a_pane_up() {
    let (mut p, left, right) = pair();

    // The grid below the header is the shell's: a drag there belongs to the terminal.
    p.drag_begin((50.0, 60.0));
    p.drag_over((140.0, 0.0));
    assert_eq!(p.drop, None);
    assert!(p.preview.is_none());
    p.drag_end(false);
    assert_eq!(order(&p.root), vec![left, right]);
}

#[test]
fn a_header_press_is_measured_from_the_tab_layout_origin() {
    let (mut p, left, right) = pair();
    p.set_origin(40.0, 50.0);
    p.set_rect(left, [40.0, 50.0, 100.0, 100.0]);
    p.set_rect(right, [140.0, 50.0, 100.0, 100.0]);

    p.drag_begin((10.0, 10.0));
    p.drag_over((130.0, 40.0));
    assert_eq!(
        p.drop,
        Some(DropTarget::Swap(right)),
        "the local press lands in the first pane's header"
    );
}

#[test]
fn a_header_drag_keeps_tracking_after_the_preview_moves_the_panes() {
    use rune::text::TextSystem;
    use rune::{Env, Runtime};

    let mut window = crate::Window::start();
    window.tabs = crate::tabs::Tabs::new(std::env::temp_dir());
    window.tabs.split(Side::Right);
    window.horizontal = false;
    window.headers = true;
    window.top_bar = false;
    window.palette = None;
    window.settings = None;

    let text = TextSystem::new(rune_theme_opencode::FONTS, rune_theme_opencode::FAMILIES);
    let mut rt = Runtime::with_text(
        window,
        crate::frame::view,
        (1000.0, 700.0),
        Env::new(true, 1.0),
        text,
    );
    rt.frame(0.0);
    rt.frame(0.0);

    let (x, y) = rt.find("block-1-name").expect("the first pane's header");
    let (rx, ry) = rt.find("block-2-name").expect("the second pane's header");

    rt.pointer_down(x, y);
    rt.frame(0.0);
    rt.pointer_move(rx, ry + 200.0);
    rt.frame(0.0);
    let preview = rt
        .state
        .tabs
        .panes_mut(1)
        .and_then(|p| p.preview.as_ref())
        .map(order)
        .expect("the swap is previewed");
    assert_eq!(preview, vec![2, 1], "the panes show their new order");

    // A second move after the header has been re-parented: a captured gesture that lost its
    // element path would never see this one, leaving the first preview on screen.
    rt.pointer_move(x, y + 300.0);
    rt.frame(0.0);
    let p = rt.state.tabs.panes_mut(1).expect("the tab");
    assert!(p.preview.is_none(), "the drag still follows the pointer");
    assert_eq!(p.drop, None, "it is back over its own pane");

    rt.pointer_move(rx, ry + 200.0);
    rt.frame(0.0);
    rt.pointer_up(rx, ry + 200.0);
    rt.frame(0.0);
    let p = rt.state.tabs.panes_mut(1).expect("the tab");
    assert!(p.preview.is_none());
    assert_eq!(order(&p.root), vec![2, 1], "the drop commits the preview");
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
