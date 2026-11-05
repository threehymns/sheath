//! What Avar's commands do: the window's own chords and the command bar's entries end up in one
//! dispatch, which reads and changes the window's state and nothing else. It sits beside the
//! window rather than in it so the crate's entry point keeps to the frame the platform drives.

use rune::Tx;

use actions_ui::Palette;

use crate::action::Action;
use crate::panes::{Dir, Side};
use crate::settings::SettingsTab;
use crate::{Window, start_dir};

/// How far one resize command moves a split: a percentage of the split the focused pane shares
/// with its neighbour on that side, so the step reads the same whatever the window's size.
const RESIZE_STEP: f32 = 5.0;

impl Window {
    /// What a command does. There is no `enabled` policy here: every command Avar has can always
    /// run, and one that found no selection simply does nothing.
    pub(crate) fn run(&mut self, action: Action, tx: &mut Tx) {
        match action {
            Action::CommandBar => self.palette = Some(Palette::open()),
            Action::OpenSettings => self.open_settings(),
            Action::NewTab => {
                self.new_tab();
                self.tab_switched(tx);
            }
            Action::CloseTab => {
                if let Some(id) = self.tabs.active_id() {
                    self.tabs.close(id);
                    self.tab_switched(tx);
                }
            }
            Action::NextTab => {
                self.tabs.cycle(1);
                self.tab_switched(tx);
            }
            Action::PrevTab => {
                self.tabs.cycle(-1);
                self.tab_switched(tx);
            }
            Action::SplitRight => self.tabs.split(Side::Right),
            Action::SplitDown => self.tabs.split(Side::Down),
            Action::ZoomPane => self.tabs.toggle_zoom(),
            Action::BalancePanes => self.tabs.balance(),
            Action::ClosePane => self.tabs.close_pane(),
            Action::FocusLeft => self.tabs.focus_dir(Dir::Left),
            Action::FocusRight => self.tabs.focus_dir(Dir::Right),
            Action::FocusUp => self.tabs.focus_dir(Dir::Up),
            Action::FocusDown => self.tabs.focus_dir(Dir::Down),
            Action::ResizeLeft => self.tabs.resize(Dir::Left, RESIZE_STEP),
            Action::ResizeRight => self.tabs.resize(Dir::Right, RESIZE_STEP),
            Action::ResizeUp => self.tabs.resize(Dir::Up, RESIZE_STEP),
            Action::ResizeDown => self.tabs.resize(Dir::Down, RESIZE_STEP),
            Action::Copy => {
                let copied = self
                    .tabs
                    .focused_mut()
                    .and_then(|term| term.copy_selection());
                if let Some(text) = copied
                    && !text.trim().is_empty()
                {
                    tx.copy(text);
                }
            }
            Action::Paste => {
                if let Ok(mut clipboard) = arboard::Clipboard::new()
                    && let Ok(text) = clipboard.get_text()
                    && let Some(term) = self.tabs.focused_mut()
                {
                    term.paste(&text);
                }
            }
            Action::SelectAll => {
                if let Some(term) = self.tabs.focused_mut() {
                    term.select_all();
                }
                self.wake_frame(tx);
            }
        }
    }

    /// A new tab, in the folder the shell the user is typing in is in.
    pub(crate) fn new_tab(&mut self) {
        let dir = self.tabs.dir().unwrap_or_else(start_dir);
        self.tabs.open(dir);
    }

    pub(crate) fn open_settings(&mut self) {
        if self.settings.is_none() {
            self.settings = Some(SettingsTab::Appearance);
        }
        self.pick = None;
    }

    pub(crate) fn open_tab(&mut self, tab: SettingsTab) {
        self.settings = Some(tab);
        self.pick = None;
    }

    pub(crate) fn close_settings(&mut self) {
        self.settings = None;
        self.pick = None;
    }
}
