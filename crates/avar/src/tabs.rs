//! Avar's tabs, each a shell and each splittable into a second pane beside it: the set of them,
//! the rail that shows them, and the grid each pane draws.
//!
//! The set owns its own invariants, so the window never has to: the tab on screen is always one
//! of the tabs, exactly one pane in the window has the keyboard, and a tab whose shell ends
//! closes itself. The rail is a rune-kit `Sidebar`, so its rows drag to reorder through the
//! kit's `Reorder`, and the split is a `SplitView`'s inspector pane, so its divider drags and
//! its pane collapses while the shell in it keeps running.

use std::path::PathBuf;
use std::time::Duration;

use rune::{Drag, El, TextStyle, Tx, Waker};
use rune_kit::shell::{PaneState, Reorder, Sidebar, SidebarRow};
use rune_kit::{Kit, glyphs};
use rune_tty::{Terminal, layout_screen, screen_of, terminal_cell, terminal_drag};

use crate::Window;

/// The tab rail's width, and the range its edge drags within.
pub const RAIL: f32 = 216.0;
pub const RAIL_MIN: f32 = 160.0;
pub const RAIL_MAX: f32 = 320.0;
/// Points between tab rows, which the reorder drag steps aside by.
pub const GAP: f32 = 4.0;
/// Tab rows are a little taller than a sidebar's default: a tab is the window's subject.
const ROW: f32 = 32.0;
/// A split pane's width, and the range its divider drags within.
pub const SPLIT: f32 = 420.0;
const SPLIT_MIN: f32 = 240.0;
const SPLIT_MAX: f32 = 960.0;

/// Which of a tab's panes: the one that fills the tab, or the split beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Main,
    Split,
}

/// Every tab the window has, in rail order, and the one on screen. Closing the last tab leaves
/// the set empty, which is how the window knows it has nothing left to show.
pub struct Tabs {
    rows: Vec<Tab>,
    active: usize,
    next: u64,
    /// The rail's drag, owned here because it is the rail's own gesture.
    drag: Reorder,
    /// The pane a frame draws in the one frame between the last tab closing and the window
    /// closing with it, so the split view always has a state to read.
    empty_pane: PaneState,
}

impl Tabs {
    pub fn new(dir: PathBuf) -> Tabs {
        Tabs {
            rows: vec![Tab::new(1, dir)],
            active: 0,
            next: 2,
            drag: Reorder::new().gap(GAP),
            empty_pane: PaneState::new(SPLIT).collapsed(),
        }
    }

    /// Nothing left to show: the window is done.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn active_id(&self) -> Option<u64> {
        self.rows.get(self.active).map(|tab| tab.id)
    }

    /// Where a shell opened from the window should start: where the shell the user is typing in
    /// is, when that shell has reported it.
    pub fn dir(&self) -> Option<PathBuf> {
        self.focused().and_then(|term| term.pwd())
    }

    /// The split pane of the tab on screen, for the split view's divider and its motion.
    pub fn pane(&self) -> PaneState {
        self.rows
            .get(self.active)
            .map_or(self.empty_pane, |tab| tab.pane)
    }

    /// The tab on screen's split pane, for the split view's divider to write to.
    pub fn pane_mut(&mut self) -> &mut PaneState {
        match self.rows.get_mut(self.active) {
            Some(tab) => &mut tab.pane,
            None => &mut self.empty_pane,
        }
    }

    pub fn split_shown(&self) -> bool {
        self.rows
            .get(self.active)
            .is_some_and(|tab| tab.pane.is_open())
    }

    /// Whether the tab on screen was ever split: its pane's content is only built once it was.
    pub fn has_split(&self) -> bool {
        self.rows.get(self.active).is_some_and(Tab::has_split)
    }

    pub fn title(&self) -> String {
        self.rows
            .get(self.active)
            .map(Tab::title)
            .unwrap_or_default()
    }

    /// The pane with the keyboard, which is the only one that takes keys or blinks.
    pub fn focused(&self) -> Option<&Terminal> {
        let tab = self.rows.get(self.active)?;
        tab.terminal(tab.focus)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Terminal> {
        let tab = self.rows.get_mut(self.active)?;
        let which = tab.focus;
        tab.terminal_mut(which)
    }

    /// Opens a tab at the end of the rail, in `dir`, and shows it.
    pub fn open(&mut self, dir: PathBuf) {
        let id = self.next;
        self.next += 1;
        self.rows.push(Tab::new(id, dir));
        self.active = self.rows.len() - 1;
        self.sync_focus();
    }

    /// Closes a tab and ends the shells that ran in it.
    pub fn close(&mut self, id: u64) {
        let Some(index) = self.rows.iter().position(|tab| tab.id == id) else {
            return;
        };
        self.rows.remove(index).close();
        if self.rows.is_empty() {
            return;
        }
        // The rail closes up around the gap: the tab after it shifts down, and the one on screen
        // stays the one on screen.
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.rows.len() - 1);
        self.sync_focus();
    }

    pub fn select(&mut self, id: u64) {
        if let Some(index) = self.rows.iter().position(|tab| tab.id == id) {
            self.active = index;
        }
        self.sync_focus();
    }

    /// A press in a pane is a press on its tab: the keyboard goes there, and that tab shows.
    pub fn focus(&mut self, id: u64, which: Which) {
        if let Some(index) = self.rows.iter().position(|tab| tab.id == id) {
            self.active = index;
            self.rows[index].focus = which;
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
        self.active = (self.active as isize + by).rem_euclid(count) as usize;
        self.sync_focus();
    }

    /// Splits the tab on screen, or closes the split and keeps its shell running.
    pub fn toggle_split(&mut self) {
        let Some(tab) = self.rows.get_mut(self.active) else {
            return;
        };
        if tab.pane.is_open() {
            tab.close_split();
        } else {
            tab.open_split();
        }
        self.sync_focus();
    }

    /// The split's shell ended, or its pane was dragged shut: the tab goes back to one pane.
    pub fn close_split(&mut self, id: u64) {
        if let Some(tab) = self.rows.iter_mut().find(|tab| tab.id == id) {
            tab.close_split();
        }
        self.sync_focus();
    }

    /// The rail: every tab as a row that drags to reorder, with a close button under the pointer
    /// and the active tab filled.
    pub fn list(&self, k: &Kit) -> El {
        let mut rail = Sidebar::new()
            .label("Tabs")
            .inset(0.0)
            .row_gap(GAP)
            .header_padding(10.0, 10.0);
        for (index, tab) in self.rows.iter().enumerate() {
            let id = tab.id;
            let row = SidebarRow::new(tab.title())
                .icon(&glyphs::CURSOR_TEXT)
                .height(ROW)
                .selected(index == self.active)
                .on_select(move |s: &mut Window, _: &mut Tx| s.tabs.select(id))
                .on_close(move |s: &mut Window, _: &mut Tx| s.tabs.close(id))
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

    /// Reads every pane's shell, on screen or not, and closes the ones whose shell ended. The
    /// window asks for the frame the rows moved in, and for the soonest cursor blink.
    pub fn poll(
        &mut self,
        mono: TextStyle,
        scale: f32,
        wake: Option<&Waker>,
        dark: bool,
        tx: &mut Tx,
    ) -> (bool, Option<Duration>) {
        let theme = rune_tty::theme(dark);
        let mut redraw = false;
        let mut blink: Option<Duration> = None;
        let mut ended: Vec<(u64, Which)> = Vec::new();
        for tab in &mut self.rows {
            let id = tab.id;
            for which in [Which::Main, Which::Split] {
                let Some(term) = tab.terminal_mut(which) else {
                    continue;
                };
                term.measure(mono, scale);
                redraw |= term.poll(wake, theme);
                for text in term.take_copies() {
                    tx.copy(text);
                }
                // The window has no toasts to put a program's OSC 9 or 777 notification in:
                // drained so they do not pile up behind it, and otherwise dropped.
                term.take_notifications();
                if term.shell_exited() {
                    ended.push((id, which));
                }
                if let Some(wait) = term.blink_wait() {
                    blink = Some(blink.map_or(wait, |soonest| soonest.min(wait)));
                }
            }
        }
        // A pane whose shell ended closes: the split first, leaving its tab, then a tab whose
        // own shell ended.
        for (id, which) in ended {
            match which {
                Which::Main => self.close(id),
                Which::Split => self.close_split(id),
            }
        }
        self.sync_focus();
        (redraw, blink)
    }

    fn terminal_mut(&mut self, id: u64, which: Which) -> Option<&mut Terminal> {
        self.rows
            .iter_mut()
            .find(|tab| tab.id == id)?
            .terminal_mut(which)
    }

    /// The keyboard goes to the active tab's focused pane and nowhere else: a shell in another
    /// tab keeps running and drawing, but its cursor stays solid and it gets no keys. A split
    /// that a drag collapsed hands the keyboard back to the tab.
    fn sync_focus(&mut self) {
        for (index, tab) in self.rows.iter_mut().enumerate() {
            if !tab.pane.is_open() && tab.focus == Which::Split {
                tab.focus = Which::Main;
            }
            let focused = (index == self.active).then_some(tab.focus);
            let main_focused = focused == Some(Which::Main);
            let split_focused = focused == Some(Which::Split);
            if let Some(term) = tab.terminal_mut(Which::Main) {
                term.set_focus(main_focused);
            }
            if let Some(term) = tab.terminal_mut(Which::Split) {
                term.set_focus(split_focused);
            }
        }
    }
}

/// One pane's grid, with its wheel, pointer, drag and layout wired to the tab and side it draws.
pub fn screen(s: &Window, id: u64, which: Which, k: &'static Kit) -> El {
    let Some(term) = s
        .tabs
        .rows
        .iter()
        .find(|tab| tab.id == id)
        .and_then(|tab| tab.terminal(which))
    else {
        return rune::el();
    };
    screen_of(term, s.scale.get(), k)
        .on_wheel(move |s: &mut Window, wheel, tx: &mut Tx| {
            // The same split the app's pane makes: a program that captured the mouse scrolls
            // its own view, and shift takes the mouse back for the history.
            let Some(term) = s.tabs.terminal_mut(id, which) else {
                return false;
            };
            let shift = tx.mods().shift;
            if !shift
                && let Some(at) = term.hovered_cell()
                && term.mouse_wheel(wheel.dy > 0.0, at, false)
            {
                return true;
            }
            term.wheel(wheel.dy)
        })
        .on_pointer_move_local(move |s: &mut Window, p, tx: &mut Tx| {
            if let Some(term) = s.tabs.terminal_mut(id, which) {
                let at = terminal_cell(term, (p.x, p.y));
                term.hover(at, tx.mods().shift);
            }
        })
        .on_hover_change(move |s: &mut Window, inside: bool, _: &mut Tx| {
            if !inside && let Some(term) = s.tabs.terminal_mut(id, which) {
                term.hover(None, false);
            }
        })
        .on_drag(move |s: &mut Window, drag: &mut Drag, tx: &mut Tx| {
            if let Some(term) = s.tabs.terminal_mut(id, which) {
                terminal_drag(term, drag, tx);
            }
        })
        // A press anywhere in the grid gives that pane the keyboard, and the tab it is in the
        // window's active one.
        .on_press(move |s: &mut Window| s.tabs.focus(id, which))
        .on_layout_tx(move |s: &mut Window, rect, tx: &mut Tx| {
            if let Some(term) = s.tabs.terminal_mut(id, which) {
                layout_screen(term, rect);
            }
            tx.live_only();
        })
}

/// One tab: the shell that fills it, and the split's shell once the tab is split.
struct Tab {
    /// What a pane's handlers name their tab by. A tab's place in the rail moves as tabs open,
    /// close and reorder, so the id is what outlives it.
    id: u64,
    main: Terminal,
    split: Option<Terminal>,
    /// The split pane's width and whether it shows; the `SplitView`'s divider writes to it.
    pane: PaneState,
    /// Which pane has the keyboard.
    focus: Which,
    /// The folder the tab started in, before its shell reported one (OSC 7).
    dir: PathBuf,
}

impl Tab {
    fn new(id: u64, dir: PathBuf) -> Tab {
        let mut main = Terminal::new();
        main.open(dir.clone());
        Tab {
            id,
            main,
            split: None,
            // A split opens on demand: it starts collapsed, keeping the width it will open at.
            pane: PaneState::new(SPLIT)
                .range(SPLIT_MIN, SPLIT_MAX)
                .collapsed(),
            focus: Which::Main,
            dir,
        }
    }

    /// Whether the tab has ever been split.
    fn has_split(&self) -> bool {
        self.split.is_some()
    }

    /// The pane's terminal. A split that was never opened has none.
    fn terminal(&self, which: Which) -> Option<&Terminal> {
        match which {
            Which::Main => Some(&self.main),
            Which::Split => self.split.as_ref(),
        }
    }

    fn terminal_mut(&mut self, which: Which) -> Option<&mut Terminal> {
        match which {
            Which::Main => Some(&mut self.main),
            Which::Split => self.split.as_mut(),
        }
    }

    /// The folder a shell opened in this tab starts in: the one the shell last reported, else
    /// the one the tab started in.
    fn dir(&self) -> PathBuf {
        self.main.pwd().unwrap_or_else(|| self.dir.clone())
    }

    /// The tab's name: the folder its shell is in, as a terminal's tab usually says.
    fn title(&self) -> String {
        let dir = self.dir();
        dir.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| dir.display().to_string())
    }

    /// Shows the split, starting its shell the first time and keeping the one it has after, and
    /// gives it the keyboard.
    fn open_split(&mut self) {
        let dir = self.dir();
        match self.split.as_mut() {
            // The shell ended while the pane was shut: the next open starts a fresh one here.
            Some(term) if !term.is_open() => term.open(dir),
            Some(_) => {}
            None => {
                let mut term = Terminal::new();
                term.open(dir);
                self.split = Some(term);
            }
        }
        self.pane.set_open(true);
        self.focus = Which::Split;
    }

    /// Hides the split and gives the keyboard back to the tab, leaving the split's shell running
    /// so opening it again is the same session.
    fn close_split(&mut self) {
        self.pane.set_open(false);
        if let Some(split) = self.split.as_mut() {
            // A shell that has ended has nothing to come back to: the next open starts a new one.
            if split.shell_exited() {
                split.close();
            } else {
                split.set_focus(false);
            }
        }
        self.focus = Which::Main;
    }

    /// Ends both shells: closing a tab ends what ran in it.
    fn close(&mut self) {
        self.main.close();
        if let Some(split) = self.split.as_mut() {
            split.close();
        }
    }
}
