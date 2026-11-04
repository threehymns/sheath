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
    SplitPane,
    Copy,
    Paste,
    SelectAll,
}

impl Action {
    pub const ALL: [Action; 10] = [
        Action::CommandBar,
        Action::OpenSettings,
        Action::NewTab,
        Action::CloseTab,
        Action::NextTab,
        Action::PrevTab,
        Action::SplitPane,
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
            Action::SplitPane => "split-pane",
            Action::Copy => "copy",
            Action::Paste => "paste",
            Action::SelectAll => "select-all",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::CommandBar => "Command Bar",
            Action::OpenSettings => "Shortcuts",
            Action::NewTab => "New Tab",
            Action::CloseTab => "Close Tab",
            Action::NextTab => "Next Tab",
            Action::PrevTab => "Previous Tab",
            Action::SplitPane => "Split Pane",
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
                Action::CloseTab => "super+w",
                Action::NextTab => "ctrl+tab",
                Action::PrevTab => "ctrl+shift+tab",
                Action::SplitPane => "super+d",
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
                // tab commands take the shifted pair a terminal usually gives them.
                Action::NewTab => "ctrl+shift+t",
                Action::CloseTab => "ctrl+shift+w",
                Action::NextTab => "ctrl+tab",
                Action::PrevTab => "ctrl+shift+tab",
                Action::SplitPane => "ctrl+shift+d",
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
            Action::SplitPane => &rune_kit::shell::INSPECTOR,
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
