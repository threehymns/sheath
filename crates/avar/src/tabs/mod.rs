//! Avar's tabs: the set of them, the rail that shows them, and the rule that exactly one block
//! in the window has the keyboard.
//!
//! A tab is Rex's window: its own layout of blocks, each block its own shell and so its own PTY.
//! This module owns the set rather than the layout — the blocks and their tree live in
//! [`crate::panes`] — so a tab here is one row of the rail plus the panes behind it, and the
//! window never has to keep the two in step. The rail is a rune-kit `Sidebar`, so its rows drag
//! to reorder through the kit's `Reorder`; the bar across the top is Avar's own, with its own
//! drag measured across, because the kit's `Reorder` only moves rows down a list. That drag and
//! the chips it moves live in [`strip`].

use std::path::PathBuf;
use std::time::Duration;

use actions::Keybinds;
use rune::{Drag, El, Phase, TextStyle, Tx, Waker, col};
use rune_kit::Kit;
use rune_kit::shell::{Reorder, Sidebar, SidebarRow};
use rune_tty::{Drain, Terminal};

use crate::Window;
use crate::action::Action;
use crate::panes::{Dir, Panes, Side};

mod strip;

use strip::Strip;

/// The tab rail's width, and the range its edge drags within.
pub const RAIL: f32 = 216.0;
pub const RAIL_MIN: f32 = 160.0;
pub const RAIL_MAX: f32 = 320.0;
/// Points between tab rows, which the reorder drag steps aside by.
pub const GAP: f32 = 4.0;
/// Tab rows are a little taller than a sidebar's default: a tab is the window's subject.
const ROW: f32 = 32.0;
/// Every tab the window has, in rail order, and the one on screen. Closing the last tab leaves
/// the set empty, which is how the window knows it has nothing left to show.
pub struct Tabs {
    rows: Vec<Tab>,
    active: usize,
    next: u64,
    /// Whether the tab on screen came from further along the rail than the one it replaced,
    /// which is the side a slide comes in from.
    forward: bool,
    /// The rail's drag, owned here because it is the rail's own gesture.
    drag: Reorder,
    /// The bar's drag, which measures its chips rather than stepping rows.
    strip: Strip,
}

/// One tab: a layout of blocks, and the folder it opened in before any shell reported one.
struct Tab {
    /// What a pane's handlers name their tab by. A tab's place in the rail moves as tabs open,
    /// close and reorder, so the id is what outlives it.
    id: u64,
    panes: Panes,
}

impl Tabs {
    pub fn new(dir: PathBuf) -> Tabs {
        Tabs {
            rows: vec![Tab {
                id: 1,
                panes: Panes::new(dir),
            }],
            active: 0,
            next: 2,
            forward: true,
            drag: Reorder::new().gap(GAP),
            strip: Strip::default(),
        }
    }

    /// Nothing left to show: the window is done.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// How many tabs the window holds: the rail or the strip is only there to choose between
    /// them, so one tab needs neither.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// One tab holding one pane: the window opens as a terminal, so it draws no rail, no
    /// toolbar and no chrome around the grid.
    pub fn is_single(&self) -> bool {
        self.rows.len() == 1 && self.rows[0].panes.blocks() == 1
    }

    pub fn active_id(&self) -> Option<u64> {
        self.rows.get(self.active).map(|tab| tab.id)
    }

    /// Whether the tab on screen sits further along the rail than the one it replaced: a slide
    /// that follows the tab order comes in from that side.
    pub fn switched_forward(&self) -> bool {
        self.forward
    }

    /// Where a shell opened from the window should start: where the shell the user is typing in
    /// is, when that shell has reported it.
    pub fn dir(&self) -> Option<PathBuf> {
        self.focused().and_then(|term| term.pwd())
    }

    pub fn title(&self) -> String {
        self.rows
            .get(self.active)
            .map(|tab| tab.panes.title())
            .unwrap_or_default()
    }

    /// The block with the keyboard, which is the only one that takes keys or blinks.
    pub fn focused(&self) -> Option<&Terminal> {
        self.rows.get(self.active)?.panes.focused()
    }

    pub fn focused_mut(&mut self) -> Option<&mut Terminal> {
        self.rows.get_mut(self.active)?.panes.focused_mut()
    }

    /// A block of any tab, for the handlers a grid wires before the tab is known to be on
    /// screen: a background tab keeps parsing what its shells write.
    pub fn block_mut(&mut self, tab: u64, block: u64) -> Option<&mut Terminal> {
        self.tab_mut(tab)
            .and_then(|row| row.panes.terminal_mut(block))
    }

    pub fn panes_mut(&mut self, tab: u64) -> Option<&mut Panes> {
        self.tab_mut(tab).map(|row| &mut row.panes)
    }

    fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.rows.iter_mut().find(|row| row.id == id)
    }

    /// Opens a tab at the end of the rail, in `dir`, and shows it.
    pub fn open(&mut self, dir: PathBuf) {
        let id = self.next;
        self.next += 1;
        self.rows.push(Tab {
            id,
            panes: Panes::new(dir),
        });
        self.active = self.rows.len() - 1;
        self.forward = true;
        self.sync_focus();
    }

    /// Closes a tab and ends the shells that ran in it.
    pub fn close(&mut self, id: u64) {
        let Some(index) = self.rows.iter().position(|tab| tab.id == id) else {
            return;
        };
        self.rows.remove(index).panes.close_all();
        if self.rows.is_empty() {
            return;
        }
        // The rail closes up around the gap: the tab after it shifts down, and the one on screen
        // stays the one on screen.
        if index < self.active {
            self.active -= 1;
            self.forward = false;
        } else if index == self.active {
            // The tab that slid into the closed one's place came from further along the rail.
            self.forward = true;
        }
        self.active = self.active.min(self.rows.len() - 1);
        self.sync_focus();
    }

    pub fn select(&mut self, id: u64) {
        if let Some(index) = self.rows.iter().position(|tab| tab.id == id) {
            self.forward = index > self.active;
            self.active = index;
        }
        self.sync_focus();
    }

    /// A press in a block is a press on its tab and its block: both take the keyboard.
    pub fn focus_block(&mut self, tab: u64, block: u64) {
        if let Some(index) = self.rows.iter().position(|row| row.id == tab) {
            self.active = index;
            self.rows[index].panes.focus(block);
        }
        self.sync_focus();
    }

    /// A press in a block's split button: that tab shows, that block is divided, and the new
    /// block takes the keyboard.
    pub fn split_at(&mut self, tab: u64, block: u64, side: Side) {
        if let Some(row) = self.tab_mut(tab) {
            row.panes.split_at(block, side);
        }
        self.sync_focus();
    }

    /// A press in a block's close button: the block and its shell go, and the tab with the last
    /// one of them.
    pub fn close_block(&mut self, tab: u64, block: u64) {
        let Some(index) = self.rows.iter().position(|row| row.id == tab) else {
            return;
        };
        let panes = &mut self.rows[index].panes;
        panes.close(block);
        let empty = panes.is_empty();
        if empty {
            self.close(tab);
        } else {
            panes.sync_focus(index == self.active);
        }
    }

    /// The window's close-pane command: the block with the keyboard goes, and so does its tab
    /// when it was the last one.
    pub fn close_pane(&mut self) {
        let Some(tab) = self.rows.get(self.active) else {
            return;
        };
        let (id, block) = (tab.id, tab.panes.focus_id());
        self.close_block(id, block);
    }

    /// Splits the focused block of the tab on screen.
    pub fn split(&mut self, side: Side) {
        if let Some(tab) = self.rows.get_mut(self.active) {
            tab.panes.split(side);
        }
        self.sync_focus();
    }

    /// Zooms the block with the keyboard, or unzooms the tab if it was already zoomed.
    pub fn toggle_zoom(&mut self) {
        if let Some(tab) = self.rows.get_mut(self.active) {
            tab.panes.toggle_zoom();
        }
    }

    /// A press in a block's zoom button: that tab shows, that block is the one zoomed.
    pub fn zoom_at(&mut self, tab: u64, block: u64) {
        if let Some(index) = self.rows.iter().position(|row| row.id == tab) {
            self.active = index;
            let panes = &mut self.rows[index].panes;
            panes.focus(block);
            if !panes.is_zoomed() {
                panes.toggle_zoom();
            }
        }
        self.sync_focus();
    }

    /// Resizes the focused block toward a direction, in the tab on screen.
    pub fn resize(&mut self, dir: Dir, amount: f32) {
        if let Some(tab) = self.rows.get_mut(self.active) {
            tab.panes.resize(dir, amount);
        }
    }

    /// Puts every split in the tab on screen back to even halves.
    pub fn balance(&mut self) {
        if let Some(tab) = self.rows.get_mut(self.active) {
            tab.panes.balance();
        }
    }

    /// Moves the keyboard to the nearest block in a direction, in the tab on screen.
    pub fn focus_dir(&mut self, dir: Dir) {
        if let Some(tab) = self.rows.get_mut(self.active) {
            tab.panes.focus_dir(dir);
        }
        self.sync_focus();
    }

    /// The rail's drag: the rows swap places, and the tab on screen stays the one on screen.
    pub fn move_row(&mut self, from: usize, to: usize) {
        if from == to || to >= self.rows.len() {
            return;
        }
        let active = self.active_id();
        let tab = self.rows.remove(from);
        self.rows.insert(to, tab);
        if let Some(id) = active
            && let Some(index) = self.rows.iter().position(|tab| tab.id == id)
        {
            self.active = index;
        }
    }

    /// The next or previous tab, wrapping at either end.
    pub fn cycle(&mut self, by: isize) {
        if self.rows.len() < 2 {
            return;
        }
        let count = self.rows.len() as isize;
        let target = (self.active as isize + by).rem_euclid(count) as usize;
        self.forward = target > self.active;
        self.active = target;
        self.sync_focus();
    }

    /// The rail: every tab as a row that drags to reorder, with a close button under the pointer
    /// and the active tab filled.
    pub fn rail(&self, k: &Kit) -> El {
        let mut rail = Sidebar::new()
            .label("Tabs")
            .inset(0.0)
            .row_gap(GAP)
            .header_padding(10.0, 10.0);
        for (index, tab) in self.rows.iter().enumerate() {
            let id = tab.id;
            let blocks = tab.panes.blocks();
            let mut row = SidebarRow::new(tab.panes.title())
                .height(ROW)
                .selected(index == self.active);
            // A split tab says how many blocks it holds, so a tab that looked away still shows
            // the work in it.
            if blocks > 1 {
                row = row.detail(format!("{blocks} panes"));
            }
            let row = row
                .on_select(move |s: &mut Window, tx: &mut Tx| {
                    s.tabs.select(id);
                    s.tab_switched(tx);
                })
                .on_close(move |s: &mut Window, tx: &mut Tx| {
                    s.tabs.close(id);
                    s.tab_switched(tx);
                })
                .reorder(
                    &self.drag,
                    index,
                    self.rows.len(),
                    |s: &mut Window| &mut s.tabs.drag,
                    |s: &mut Window, from, to| s.tabs.move_row(from, to),
                )
                .build(k)
                .key(format!("tab-{id}"));
            rail = rail.child(row);
        }
        rail.build(k)
    }

    /// A tab's layout, ready to draw: the blocks of the tab on screen, split as it was left.
    ///
    /// The wrapper owns the header drag rather than the header itself. A preview re-parents the
    /// panes, and Rune captures a gesture by the element's path: a header that moved would stop
    /// resolving and the drag would freeze, so the gesture sits on this box instead, which stays
    /// put for the whole drag.
    pub fn layout(
        &self,
        tab: u64,
        scale: f32,
        k: &'static Kit,
        headers: bool,
        binds: &Keybinds<Action>,
    ) -> El {
        let Some(row) = self.rows.iter().find(|row| row.id == tab) else {
            return rune::el();
        };
        let mut shell = col()
            .key(format!("panes-{tab}"))
            .w_full()
            .h_full()
            .min_h(0.0)
            .on_layout_tx(move |s: &mut Window, rect, _: &mut Tx| {
                if let Some(p) = s.tabs.panes_mut(tab) {
                    p.set_origin(rect[0], rect[1]);
                }
            });
        if headers {
            shell = shell.on_drag(move |s: &mut Window, d: &mut Drag, tx: &mut Tx| {
                let Some(p) = s.tabs.panes_mut(tab) else {
                    return;
                };
                match d.phase {
                    Phase::Down => p.drag_begin((d.x, d.y)),
                    Phase::Move => p.drag_over((d.dx, d.dy)),
                    Phase::Up => p.drag_end(d.cancelled()),
                }
                // The previewed layout and the drop land at once, so the panes follow the
                // pointer instead of gliding behind it.
                tx.instant();
            });
        }
        shell.child(row.panes.layout(tab, scale, k, headers, binds))
    }

    /// Reads every block's shell, on screen or not, and closes the blocks whose shell ended. The
    /// window asks for the frame the rows moved in, and for the soonest cursor blink.
    pub fn poll(
        &mut self,
        mono: TextStyle,
        scale: f32,
        wake: Option<&Waker>,
        dark: bool,
        tx: &mut Tx,
    ) -> (bool, Option<Duration>) {
        let mut drain = Drain::default();
        let mut ended: Vec<(u64, u64)> = Vec::new();
        for tab in &mut self.rows {
            let (tab_drain, closed) = tab.panes.poll(mono, scale, wake, dark, tx);
            drain.merge(tab_drain);
            for block in closed {
                ended.push((tab.id, block));
            }
        }
        // A block whose shell ended closes, and a tab with no blocks left closes with it, so the
        // window closes when its last shell ends, the way Ghostty's does.
        for (tab, block) in ended {
            self.close_block(tab, block);
        }
        self.sync_focus();
        (drain.redraw, drain.blink)
    }

    /// The keyboard goes to the active tab's focused block and nowhere else: a shell in another
    /// tab keeps running and drawing, but its cursor stays solid and it gets no keys.
    fn sync_focus(&mut self) {
        for (index, tab) in self.rows.iter_mut().enumerate() {
            tab.panes.sync_focus(index == self.active);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The side a slide comes in from is the side the tab sits on: opening one puts it after
    /// the tab on screen, picking the first moves back up the rail, and cycling follows the
    /// direction of the key.
    #[test]
    fn a_switch_says_which_way_along_the_rail_it_went() {
        let dir = std::env::temp_dir();
        let mut tabs = Tabs::new(dir.clone());
        tabs.open(dir);
        assert!(
            tabs.switched_forward(),
            "a new tab opens at the end of the rail"
        );
        tabs.select(tabs.rows[0].id);
        assert!(!tabs.switched_forward(), "the first tab sits before it");
        tabs.cycle(1);
        assert!(tabs.switched_forward(), "the next tab is further along");
        tabs.cycle(-1);
        assert!(!tabs.switched_forward(), "the previous one is back up");
    }
}
