//! Avar's commands. The enum stays here so `Window::run` is an exhaustive match, and everything
//! the shared keymap needs about them is in the `actions::Action` implementation below.

use rune::Icon;
use rune_kit::{Kit, glyphs};
use rune_theme_opencode as theme;

use crate::icons;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    CommandBar,
    OpenSettings,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SplitRight,
    SplitDown,
    ZoomPane,
    BalancePanes,
    ClosePane,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    ResizeLeft,
    ResizeRight,
    ResizeUp,
    ResizeDown,
    Copy,
    Paste,
    SelectAll,
}

impl Action {
    pub const ALL: [Action; 22] = [
        Action::CommandBar,
        Action::OpenSettings,
        Action::NewTab,
        Action::CloseTab,
        Action::NextTab,
        Action::PrevTab,
        Action::SplitRight,
        Action::SplitDown,
        Action::ZoomPane,
        Action::BalancePanes,
        Action::ClosePane,
        Action::FocusLeft,
        Action::FocusRight,
        Action::FocusUp,
        Action::FocusDown,
        Action::ResizeLeft,
        Action::ResizeRight,
        Action::ResizeUp,
        Action::ResizeDown,
        Action::Copy,
        Action::Paste,
        Action::SelectAll,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Action::CommandBar => "command-bar",
            Action::OpenSettings => "open-settings",
            Action::NewTab => "new-tab",
            Action::CloseTab => "close-tab",
            Action::NextTab => "next-tab",
            Action::PrevTab => "prev-tab",
            Action::SplitRight => "split-right",
            Action::SplitDown => "split-down",
            Action::ZoomPane => "zoom-pane",
            Action::BalancePanes => "balance-panes",
            Action::ClosePane => "close-pane",
            Action::FocusLeft => "focus-left",
            Action::FocusRight => "focus-right",
            Action::FocusUp => "focus-up",
            Action::FocusDown => "focus-down",
            Action::ResizeLeft => "resize-left",
            Action::ResizeRight => "resize-right",
            Action::ResizeUp => "resize-up",
            Action::ResizeDown => "resize-down",
            Action::Copy => "copy",
            Action::Paste => "paste",
            Action::SelectAll => "select-all",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::CommandBar => "Command Bar",
            Action::OpenSettings => "Settings",
            Action::NewTab => "New Tab",
            Action::CloseTab => "Close Tab",
            Action::NextTab => "Next Tab",
            Action::PrevTab => "Previous Tab",
            Action::SplitRight => "Split Right",
            Action::SplitDown => "Split Down",
            Action::ZoomPane => "Zoom Pane",
            Action::BalancePanes => "Balance Panes",
            Action::ClosePane => "Close Pane",
            Action::FocusLeft => "Focus Pane Left",
            Action::FocusRight => "Focus Pane Right",
            Action::FocusUp => "Focus Pane Up",
            Action::FocusDown => "Focus Pane Down",
            Action::ResizeLeft => "Resize Pane Left",
            Action::ResizeRight => "Resize Pane Right",
            Action::ResizeUp => "Resize Pane Up",
            Action::ResizeDown => "Resize Pane Down",
            Action::Copy => "Copy",
            Action::Paste => "Paste",
            Action::SelectAll => "Select All",
        }
    }

    pub fn default_chord(self) -> Option<&'static str> {
        Some(self.builtin_chord())
    }

    fn builtin_chord(self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            match self {
                Action::CommandBar => "ctrl+p",
                Action::OpenSettings => "super+comma",
                Action::NewTab => "super+t",
                // The pane's close is the unshifted chord a terminal gives it, and the tab's is
                // the shifted one beside it: closing a tab is the bigger gesture.
                Action::ClosePane => "super+w",
                Action::CloseTab => "shift+super+w",
                Action::NextTab => "ctrl+tab",
                Action::PrevTab => "ctrl+shift+tab",
                Action::SplitRight => "super+d",
                Action::SplitDown => "shift+super+d",
                // Rex zooms the focused pane with Cmd-E, and the window gives it the same key.
                Action::ZoomPane => "super+e",
                Action::BalancePanes => "ctrl+alt+0",
                Action::FocusLeft => "ctrl+alt+left",
                Action::FocusRight => "ctrl+alt+right",
                Action::FocusUp => "ctrl+alt+up",
                Action::FocusDown => "ctrl+alt+down",
                // The focused pane grows toward the arrow, as Rex's `pane.resize` does.
                Action::ResizeLeft => "ctrl+alt+shift+left",
                Action::ResizeRight => "ctrl+alt+shift+right",
                Action::ResizeUp => "ctrl+alt+shift+up",
                Action::ResizeDown => "ctrl+alt+shift+down",
                // A shell is typing in the window, so a bare Ctrl/Cmd-C is the shell's: the
                // window's own copy and paste are the shifted ones, as the app's pane has them.
                Action::Copy => "shift+super+c",
                Action::Paste => "shift+super+v",
                Action::SelectAll => "shift+super+a",
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            match self {
                Action::CommandBar => "ctrl+p",
                Action::OpenSettings => "ctrl+comma",
                // Ctrl-T and Ctrl-W are the shell's own keys (transpose, erase word), so the
                // window's commands take the shifted pair a terminal usually gives them.
                Action::NewTab => "ctrl+shift+t",
                Action::ClosePane => "ctrl+shift+w",
                Action::CloseTab => "ctrl+shift+x",
                Action::NextTab => "ctrl+tab",
                Action::PrevTab => "ctrl+shift+tab",
                Action::SplitRight => "ctrl+shift+d",
                Action::SplitDown => "ctrl+shift+e",
                Action::ZoomPane => "ctrl+shift+z",
                Action::BalancePanes => "ctrl+alt+0",
                Action::FocusLeft => "ctrl+alt+left",
                Action::FocusRight => "ctrl+alt+right",
                Action::FocusUp => "ctrl+alt+up",
                Action::FocusDown => "ctrl+alt+down",
                // The focused pane grows toward the arrow, as Rex's `pane.resize` does.
                Action::ResizeLeft => "ctrl+alt+shift+left",
                Action::ResizeRight => "ctrl+alt+shift+right",
                Action::ResizeUp => "ctrl+alt+shift+up",
                Action::ResizeDown => "ctrl+alt+shift+down",
                Action::Copy => "shift+ctrl+c",
                Action::Paste => "shift+ctrl+v",
                Action::SelectAll => "shift+ctrl+a",
            }
        }
    }

    pub fn icon(self, _k: &Kit) -> &'static Icon {
        match self {
            Action::CommandBar => &glyphs::KEYBOARD,
            Action::OpenSettings => &theme::icons::SETTINGS_GEAR,
            Action::NewTab => &icons::PLUS,
            Action::CloseTab => &glyphs::CLOSE,
            Action::NextTab => &glyphs::ARROW_RIGHT,
            Action::PrevTab => &icons::ARROW_LEFT,
            Action::SplitRight => &icons::SPLIT_RIGHT,
            Action::SplitDown => &icons::SPLIT_DOWN,
            Action::ZoomPane => &icons::ZOOM,
            Action::BalancePanes => &icons::BALANCE,
            Action::ClosePane => &glyphs::CLOSE,
            Action::FocusLeft => &icons::ARROW_LEFT,
            Action::FocusRight => &glyphs::ARROW_RIGHT,
            Action::FocusUp => &icons::ARROW_UP,
            Action::FocusDown => &icons::ARROW_DOWN,
            Action::ResizeLeft => &icons::ARROW_LEFT,
            Action::ResizeRight => &glyphs::ARROW_RIGHT,
            Action::ResizeUp => &icons::ARROW_UP,
            Action::ResizeDown => &icons::ARROW_DOWN,
            Action::Copy => &theme::icons::COPY,
            Action::Paste => &icons::CLIPBOARD,
            Action::SelectAll => &icons::CHECK_SQUARE,
        }
    }
}

impl actions::Action for Action {
    const ALL: &'static [Self] = &Action::ALL;

    fn from_id(id: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|action| action.id() == id)
    }

    fn id(self) -> &'static str {
        Action::id(self)
    }

    fn label(self) -> &'static str {
        Action::label(self)
    }

    fn default_chord(self) -> Option<&'static str> {
        Action::default_chord(self)
    }

    /// The command bar lists the commands; it does not list itself.
    fn in_palette(self) -> bool {
        self != Action::CommandBar
    }

    /// The palette is the same chord whether the user reaches for it with a capital letter.
    fn ignores_shift(self) -> bool {
        self == Action::CommandBar
    }
}
