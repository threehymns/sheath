//! One tab's blocks: the shells it holds, the tree of splits that tiles them, and the rule that
//! keeps exactly one of them focused.
//!
//! This is Rex's model without the server: a window (here a tab) holds a layout of blocks, every
//! block is its own terminal and so its own PTY, and a split puts a new block beside the focused
//! one rather than replacing anything. Closing a block takes its split down with it and gives its
//! sibling the space, so the tree only ever holds the blocks that are still running.

use std::path::PathBuf;
use std::time::Duration;

use actions::Keybinds;
use rune::{El, TextStyle, Tx, Waker};
use rune_kit::Kit;
use rune_tty::Terminal;

use crate::action::Action;

mod drag;
mod tree;
mod view;

pub use drag::DropTarget;
#[cfg(test)]
pub use drag::Edge;

use tree::{Node, balance_node, path_to, place, remove, split_parts, split_side};

/// Which way a split puts its new block beside the focused one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Right,
    Down,
}

/// A direction the keyboard travels between blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// The smallest share of a split either side may take, so a divider can always be grabbed again.
const MIN_SHARE: f32 = 0.18;
/// Points of gap between two blocks: the gap is the divider the pointer drags.
pub const GAP: f32 = 6.0;
/// Points of a pane's header: the strip above its shell that picks the pane up.
pub(super) const HEADER: f32 = 28.0;

/// A shell in a tab, with the rect it was last laid out in: a directional focus needs the rects.
pub struct Block {
    pub id: u64,
    term: Terminal,
    rect: [f32; 4],
}

/// The divider being dragged: the split it belongs to and the share the drag started from, so a
/// pointer that leaves and comes back lands where it left.
struct Resizing {
    split: u64,
    share: f32,
}

pub struct Panes {
    blocks: Vec<Block>,
    root: Node,
    /// The block with the keyboard, and the one a split divides.
    focus: u64,
    /// One block filling the tab, Rex's zoom: the others keep running out of sight.
    zoomed: bool,
    /// Ids for blocks and splits alike: the two never share a tree, and one counter keeps every
    /// name in the tab distinct.
    next: u64,
    /// The split whose divider is being dragged, if any.
    resizing: Option<Resizing>,
    /// The block a header drag has picked up, where the press landed, and where letting it go
    /// would put the block.
    dragging: Option<u64>,
    press: Option<(f32, f32)>,
    drop: Option<DropTarget>,
    /// The layout the drop would leave, drawn in place of the committed tree until the release.
    preview: Option<Node>,
    /// Where the tab's layout starts in the window, which turns a drag's local press into the
    /// window points the panes' rects are in.
    origin: (f32, f32),
    /// The folder a shell opened from this tab starts in before one reports its own (OSC 7).
    dir: PathBuf,
}

impl Panes {
    pub fn new(dir: PathBuf) -> Panes {
        let mut term = Terminal::new();
        term.open(dir.clone());
        Panes {
            blocks: vec![Block {
                id: 1,
                term,
                rect: [0.0; 4],
            }],
            root: Node::Block(1),
            focus: 1,
            zoomed: false,
            next: 2,
            resizing: None,
            dragging: None,
            press: None,
            drop: None,
            preview: None,
            origin: (0.0, 0.0),
            dir,
        }
    }

    /// No blocks left: the tab has nothing to show and closes.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn block(&self, id: u64) -> Option<&Block> {
        self.blocks.iter().find(|block| block.id == id)
    }

    /// How many blocks the tab holds, for its row in the rail.
    pub fn blocks(&self) -> usize {
        self.blocks.len()
    }

    pub fn focuses(&self, id: u64) -> bool {
        self.focus == id
    }

    /// The block a command aimed at "the pane" means: the one with the keyboard.
    pub fn focus_id(&self) -> u64 {
        self.focus
    }

    pub fn focused(&self) -> Option<&Terminal> {
        self.block(self.focus).map(|block| &block.term)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Terminal> {
        let id = self.focus;
        self.terminal_mut(id)
    }

    pub fn terminal_mut(&mut self, id: u64) -> Option<&mut Terminal> {
        self.blocks
            .iter_mut()
            .find(|block| block.id == id)
            .map(|block| &mut block.term)
    }

    pub fn is_zoomed(&self) -> bool {
        self.zoomed
    }

    /// The tab's name, as a block's header says it.
    pub fn title(&self) -> String {
        self.name(self.focus)
    }

    /// What a block's header calls it: the title its program set with OSC 0/2, or the folder its
    /// shell is in when the program has set none.
    pub fn name(&self, id: u64) -> String {
        if let Some(title) = self
            .block(id)
            .map(|block| block.term.frame.title.trim())
            .filter(|title| !title.is_empty())
        {
            return title.to_owned();
        }
        let dir = self
            .block(id)
            .and_then(|block| block.term.pwd())
            .unwrap_or_else(|| self.dir.clone());
        dir.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| dir.display().to_string())
    }

    /// Splits the focused block, which keeps its place and its shell; the new block goes to the
    /// side asked for and takes the keyboard.
    pub fn split(&mut self, side: Side) {
        let id = self.focus;
        self.split_at(id, side);
    }

    /// Splits `id`, from a block's own header: the block it was pressed on is the one divided.
    pub fn split_at(&mut self, id: u64, side: Side) {
        let Some(dir) = self
            .block(id)
            .map(|block| block.term.pwd().unwrap_or_else(|| self.dir.clone()))
        else {
            return;
        };
        let block = self.next;
        self.next += 1;
        let split = self.next;
        self.next += 1;
        let mut term = Terminal::new();
        term.open(dir);
        // The old block keeps the keyboard until the new one is in the tree; both start unfocused
        // so a frame drawn between the two never shows two cursors.
        term.set_focus(false);
        self.blocks.push(Block {
            id: block,
            term,
            rect: [0.0; 4],
        });
        let divided = Node::Split {
            id: split,
            side,
            share: 0.5,
            length: 0.0,
            first: Box::new(Node::Block(id)),
            second: Box::new(Node::Block(block)),
        };
        place(&mut self.root, id, divided);
        // A split is how the other blocks become visible again.
        self.zoomed = false;
        self.focus = block;
    }

    /// Closes a block and its shell, and the last one closes the tab: its owner does that when
    /// [`Panes::is_empty`] says so.
    pub fn close(&mut self, id: u64) {
        if self.blocks.len() <= 1 {
            self.close_all();
            return;
        }
        let Some(index) = self.blocks.iter().position(|block| block.id == id) else {
            return;
        };
        self.blocks.remove(index).term.close();
        if self.dragging == Some(id) || self.drop.is_some_and(|drop| drop.pane() == id) {
            self.dragging = None;
            self.press = None;
            self.drop = None;
            self.preview = None;
        }
        // The tree is rebuilt from what is left: `remove` takes the node it walks, and the
        // placeholder is never drawn because the last block was handled above.
        let root = std::mem::replace(&mut self.root, Node::Block(0));
        if let Some(root) = remove(root, id) {
            self.root = root;
        }
        self.zoomed = false;
        // The keyboard goes to the block that took the closed one's place: the one after it in
        // the layout, or the last one when it was the end.
        self.focus = self
            .blocks
            .get(index)
            .or_else(|| self.blocks.last())
            .map_or(0, |block| block.id);
    }

    /// Ends every shell, whatever the layout holds.
    pub fn close_all(&mut self) {
        for block in &mut self.blocks {
            block.term.close();
        }
        self.blocks.clear();
        self.zoomed = false;
        self.resizing = None;
        self.dragging = None;
        self.press = None;
        self.drop = None;
        self.preview = None;
    }

    /// Gives the keyboard to a block, which is how a press in a grid says which shell to type at.
    pub fn focus(&mut self, id: u64) {
        if self.block(id).is_some() {
            self.focus = id;
        }
    }

    /// Moves the keyboard to the nearest block in a direction, by the rects the last frame laid
    /// out: what a grid of splits means by "the pane to the left".
    pub fn focus_dir(&mut self, dir: Dir) {
        let Some(current) = self.block(self.focus) else {
            return;
        };
        let [x, y, w, h] = current.rect;
        let (cx, cy) = (x + w * 0.5, y + h * 0.5);
        let mut best: Option<(f32, u64)> = None;
        for block in &self.blocks {
            if block.id == self.focus {
                continue;
            }
            let [bx, by, bw, bh] = block.rect;
            let (bcx, bcy) = (bx + bw * 0.5, by + bh * 0.5);
            let (along, across, ahead) = match dir {
                Dir::Left => (cx - bcx, (cy - bcy).abs(), bcx < cx),
                Dir::Right => (bcx - cx, (cy - bcy).abs(), bcx > cx),
                Dir::Up => (cy - bcy, (cx - bcx).abs(), bcy < cy),
                Dir::Down => (bcy - cy, (cx - bcx).abs(), bcy > cy),
            };
            if !ahead {
                continue;
            }
            // Straight ahead wins over diagonal: an off-axis step counts half.
            let score = along + across * 0.5;
            if best.is_none_or(|(best, _)| score < best) {
                best = Some((score, block.id));
            }
        }
        if let Some((_, id)) = best {
            self.focus = id;
            self.zoomed = false;
        }
    }

    /// Rex's zoom: the focused block takes the tab and the others keep running out of sight.
    pub fn toggle_zoom(&mut self) {
        self.zoomed = !self.zoomed;
        // A zoomed tab has no second pane to drop a header on.
        self.dragging = None;
        self.press = None;
        self.drop = None;
        self.preview = None;
    }

    /// Rex's `pane.resize`: the focused block grows toward `dir` by `amount` percent of the split
    /// it shares with its neighbour on that side. The boundary that moves is the deepest one on
    /// the direction's axis with a block on the far side, so a resize acts on the pane the user
    /// can see rather than on the tab's outermost split. A block that already reaches that edge
    /// of the tab has no boundary to move, and nothing happens.
    pub fn resize(&mut self, dir: Dir, amount: f32) {
        let mut path = Vec::new();
        if !path_to(&self.root, self.focus, &mut path) {
            return;
        }
        // Left and up mean the block is the split's second child: the neighbour is the first.
        let (side, in_second) = match dir {
            Dir::Left => (Side::Right, true),
            Dir::Right => (Side::Right, false),
            Dir::Up => (Side::Down, true),
            Dir::Down => (Side::Down, false),
        };
        let Some(split) = path.iter().rev().find_map(|(split, second)| {
            (split_side(&self.root, *split) == Some(side) && *second == in_second).then_some(*split)
        }) else {
            return;
        };
        let Some((share, _)) = split_parts(&mut self.root, split) else {
            return;
        };
        // The split's share is the first child's part, so a block that is the second child grows
        // by taking from it.
        let step = amount.max(0.0) / 100.0;
        let wanted = if in_second {
            *share - step
        } else {
            *share + step
        };
        *share = wanted.clamp(MIN_SHARE, 1.0 - MIN_SHARE);
    }

    /// Rex's `pane.balance`: every split in the tab goes back to even halves.
    pub fn balance(&mut self) {
        balance_node(&mut self.root);
        self.resizing = None;
    }

    /// Records where the tab's layout starts in the window, which a header drag adds its local
    /// press to before it asks which pane was picked up.
    pub fn set_origin(&mut self, x: f32, y: f32) {
        self.origin = (x, y);
    }

    /// Records where a block was laid out, for a directional focus. While a header drag previews
    /// a layout the panes move, but the drag keeps aiming at the layout it started in, so the
    /// rects stay as the drop aimed at them.
    pub fn set_rect(&mut self, id: u64, rect: [f32; 4]) {
        if self.dragging.is_some() {
            return;
        }
        if let Some(block) = self.blocks.iter_mut().find(|block| block.id == id) {
            block.rect = rect;
        }
    }

    /// Records how long a split is along its axis, which a drag divides points by.
    pub fn set_length(&mut self, id: u64, length: f32) {
        if let Some((_, at)) = split_parts(&mut self.root, id) {
            *at = length.max(1.0);
        }
    }

    /// A press on a divider: the drag starts from the share the split has now.
    pub fn begin_resize(&mut self, split: u64) {
        let Some((share, _)) = split_parts(&mut self.root, split) else {
            return;
        };
        let share = *share;
        self.resizing = Some(Resizing { split, share });
    }

    /// The pointer has moved `delta` points along the split's axis since the press.
    pub fn drag_resize(&mut self, split: u64, delta: f32) {
        let Some(share_at) = self
            .resizing
            .as_ref()
            .filter(|r| r.split == split)
            .map(|r| r.share)
        else {
            return;
        };
        let Some((share, length)) = split_parts(&mut self.root, split) else {
            return;
        };
        let wanted = share_at + delta / *length;
        *share = wanted.clamp(MIN_SHARE, 1.0 - MIN_SHARE);
    }

    pub fn end_resize(&mut self) {
        self.resizing = None;
    }

    /// Double-clicking a divider puts the split back to half, as Rex does.
    pub fn reset_split(&mut self, split: u64) {
        if let Some((share, _)) = split_parts(&mut self.root, split) {
            *share = 0.5;
        }
    }

    /// Reads every block's shell, on screen or not, and reports the ones whose shell ended.
    pub fn poll(
        &mut self,
        mono: TextStyle,
        scale: f32,
        wake: Option<&Waker>,
        dark: bool,
        tx: &mut Tx,
    ) -> (bool, Option<Duration>, Vec<u64>) {
        let theme = rune_tty::theme(dark);
        let mut redraw = false;
        let mut blink: Option<Duration> = None;
        let mut ended = Vec::new();
        for block in &mut self.blocks {
            block.term.measure(mono, scale);
            redraw |= block.term.poll(wake, theme);
            for text in block.term.take_copies() {
                tx.copy(text);
            }
            // The window has no toasts to put a program's OSC 9 or 777 notification in: drained
            // so they do not pile up behind it, and otherwise dropped.
            block.term.take_notifications();
            if block.term.shell_exited() {
                ended.push(block.id);
            }
            if let Some(wait) = block.term.blink_wait() {
                blink = Some(blink.map_or(wait, |soonest| soonest.min(wait)));
            }
        }
        (redraw, blink, ended)
    }

    /// The tab's layout: the blocks as panes, split by the gaps that resize them. A tab holding
    /// one block with no headers draws just its grid: a window that is one shell is a terminal,
    /// not a pane. A zoomed pane keeps its place in the tree and takes the whole share of every
    /// split on its path, so it glides to the tab's edge on the layout spring.
    pub fn layout(
        &self,
        tab: u64,
        scale: f32,
        k: &'static Kit,
        headers: bool,
        binds: &Keybinds<Action>,
    ) -> El {
        let chrome = view::Chrome {
            headers,
            zoom: self.zoomed.then_some(self.focus),
            binds,
        };
        if self.blocks.len() == 1 {
            return match self.block(self.focus) {
                Some(block) if !headers => view::bare(tab, block, scale, k),
                Some(block) => view::pane(self, tab, block, scale, k, &chrome),
                None => rune::el(),
            };
        }
        let root = self.preview.as_ref().unwrap_or(&self.root);
        view::tree(self, tab, root, scale, k, &chrome)
    }

    /// Puts the keyboard on the active block and takes it off every other one.
    pub fn sync_focus(&mut self, active: bool) {
        for block in &mut self.blocks {
            block.term.set_focus(active && block.id == self.focus);
        }
    }
}

#[cfg(test)]
mod tests;
