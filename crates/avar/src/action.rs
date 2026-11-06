//! Avar's commands. The enum and its metadata live in one declaration; `Window::run` is the
//! exhaustive match that decides what each one does.

use rune::Icon;
use rune_kit::{Kit, glyphs};
use rune_theme_opencode as theme;

use crate::icons;

actions::declare_actions! {
    pub enum Action {
        CommandBar = "command-bar", "Command Bar", chord("ctrl+p", "ctrl+p"),
        OpenSettings = "open-settings", "Settings", chord("ctrl+comma", "super+comma"),
        NewTab = "new-tab", "New Tab", chord("ctrl+shift+t", "super+t"),
        CloseTab = "close-tab", "Close Tab", chord("ctrl+shift+x", "shift+super+w"),
        NextTab = "next-tab", "Next Tab", chord("ctrl+tab", "ctrl+tab"),
        PrevTab = "prev-tab", "Previous Tab", chord("ctrl+shift+tab", "ctrl+shift+tab"),
        SplitRight = "split-right", "Split Right", chord("ctrl+shift+d", "super+d"),
        SplitDown = "split-down", "Split Down", chord("ctrl+shift+e", "shift+super+d"),
        ZoomPane = "zoom-pane", "Zoom Pane", chord("ctrl+shift+z", "super+e"),
        BalancePanes = "balance-panes", "Balance Panes", chord("ctrl+alt+0", "ctrl+alt+0"),
        ClosePane = "close-pane", "Close Pane", chord("ctrl+shift+w", "super+w"),
        FocusLeft = "focus-left", "Focus Pane Left", chord("ctrl+alt+left", "ctrl+alt+left"),
        FocusRight = "focus-right", "Focus Pane Right", chord("ctrl+alt+right", "ctrl+alt+right"),
        FocusUp = "focus-up", "Focus Pane Up", chord("ctrl+alt+up", "ctrl+alt+up"),
        FocusDown = "focus-down", "Focus Pane Down", chord("ctrl+alt+down", "ctrl+alt+down"),
        ResizeLeft = "resize-left", "Resize Pane Left", chord("ctrl+alt+shift+left", "ctrl+alt+shift+left"),
        ResizeRight = "resize-right", "Resize Pane Right", chord("ctrl+alt+shift+right", "ctrl+alt+shift+right"),
        ResizeUp = "resize-up", "Resize Pane Up", chord("ctrl+alt+shift+up", "ctrl+alt+shift+up"),
        ResizeDown = "resize-down", "Resize Pane Down", chord("ctrl+alt+shift+down", "ctrl+alt+shift+down"),
        Copy = "copy", "Copy", chord("shift+ctrl+c", "shift+super+c"),
        Paste = "paste", "Paste", chord("shift+ctrl+v", "shift+super+v"),
        SelectAll = "select-all", "Select All", chord("shift+ctrl+a", "shift+super+a"),
    }

    impl {
        /// The command bar lists the commands; it does not list itself.
        fn in_palette(self) -> bool {
            self != Action::CommandBar
        }

        /// The palette is the same chord whether the user reaches for it with a capital letter.
        fn ignores_shift(self) -> bool {
            self == Action::CommandBar
        }
    }
}

impl Action {
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

    /// Whether the chord reaches the window while a shell has the keyboard. Copy, paste and
    /// select-all belong to the shell then: its own Ctrl/Cmd chords are what a terminal user
    /// expects, so the window's shifted pair is the one it sends to the pane instead.
    pub fn window_chord(self) -> bool {
        match self {
            Action::Copy | Action::Paste | Action::SelectAll => false,
            Action::CommandBar
            | Action::OpenSettings
            | Action::NewTab
            | Action::CloseTab
            | Action::NextTab
            | Action::PrevTab
            | Action::SplitRight
            | Action::SplitDown
            | Action::ZoomPane
            | Action::BalancePanes
            | Action::ClosePane
            | Action::FocusLeft
            | Action::FocusRight
            | Action::FocusUp
            | Action::FocusDown
            | Action::ResizeLeft
            | Action::ResizeRight
            | Action::ResizeUp
            | Action::ResizeDown => true,
        }
    }
}
