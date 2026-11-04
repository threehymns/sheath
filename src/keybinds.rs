use rune::Icon;
use rune_kit::{Kit, glyphs};
use rune_theme_opencode as theme;

use crate::icons;

/// The app's commands. The enum stays here so `perform` is an exhaustive match; everything the
/// keymap needs about them is in the shared `actions::Action` implementation below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    CommandBar,
    FocusSearch,
    OpenSettings,
    Back,
    Forward,
    EnclosingFolder,
    GoToFolder,
    Refresh,
    NewFolder,
    NewFile,
    ToggleHidden,
    Rename,
    Move,
    Trash,
    Delete,
    Copy,
    CopyPath,
    Cut,
    Paste,
    Duplicate,
    SelectAll,
    DeselectAll,
    InvertSelection,
    ViewFolderRules,
    ViewEntryRules,
    SaveLayoutRule,
    ShowEntry,
    HideEntry,
    PickRule,
    OpenTerminal,
    Reveal,
    CopyName,
    AddPlace,
    MakeAlias,
    ToggleLayout,
    ToggleInspector,
    ToggleFoldersFirst,
}

/// The shared keymap, with this app's action type.
pub type Keybinds = actions::Keybinds<Action>;

impl Action {
    pub const ALL: [Action; 37] = [
        Action::CommandBar,
        Action::FocusSearch,
        Action::OpenSettings,
        Action::Back,
        Action::Forward,
        Action::EnclosingFolder,
        Action::Refresh,
        Action::NewFolder,
        Action::ToggleHidden,
        Action::Rename,
        Action::Move,
        Action::Trash,
        Action::Delete,
        Action::Copy,
        Action::Cut,
        Action::Paste,
        Action::Duplicate,
        Action::SelectAll,
        Action::DeselectAll,
        Action::InvertSelection,
        Action::GoToFolder,
        Action::NewFile,
        Action::CopyPath,
        Action::ViewFolderRules,
        Action::ViewEntryRules,
        Action::SaveLayoutRule,
        Action::ShowEntry,
        Action::HideEntry,
        Action::PickRule,
        Action::OpenTerminal,
        Action::Reveal,
        Action::CopyName,
        Action::AddPlace,
        Action::MakeAlias,
        Action::ToggleLayout,
        Action::ToggleInspector,
        Action::ToggleFoldersFirst,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Action::CommandBar => "command-bar",
            Action::FocusSearch => "focus-search",
            Action::OpenSettings => "open-settings",
            Action::Back => "back",
            Action::Forward => "forward",
            Action::EnclosingFolder => "enclosing-folder",
            Action::GoToFolder => "go-to-folder",
            Action::Refresh => "refresh",
            Action::NewFolder => "new-folder",
            Action::NewFile => "new-file",
            Action::ToggleHidden => "toggle-hidden",
            Action::Rename => "rename",
            Action::Move => "move",
            Action::Trash => "trash",
            Action::Delete => "delete",
            Action::Copy => "copy",
            Action::CopyPath => "copy-path",
            Action::Cut => "cut",
            Action::Paste => "paste",
            Action::Duplicate => "duplicate",
            Action::SelectAll => "select-all",
            Action::DeselectAll => "deselect-all",
            Action::InvertSelection => "invert-selection",
            Action::ViewFolderRules => "view-folder-rules",
            Action::ViewEntryRules => "view-entry-rules",
            Action::SaveLayoutRule => "save-layout-rule",
            Action::ShowEntry => "show-entry",
            Action::HideEntry => "hide-entry",
            Action::PickRule => "pick-rule",
            Action::OpenTerminal => "open-terminal",
            Action::Reveal => "reveal",
            Action::CopyName => "copy-name",
            Action::AddPlace => "add-place",
            Action::MakeAlias => "make-alias",
            Action::ToggleLayout => "toggle-layout",
            Action::ToggleInspector => "toggle-inspector",
            Action::ToggleFoldersFirst => "toggle-folders-first",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::CommandBar => "Command Bar",
            Action::FocusSearch => "Search This Folder",
            Action::OpenSettings => "Settings",
            Action::Back => "Back",
            Action::Forward => "Forward",
            Action::EnclosingFolder => "Enclosing Folder",
            Action::GoToFolder => "Go to Folder…",
            Action::Refresh => "Refresh",
            Action::NewFolder => "New Folder",
            Action::NewFile => "New File",
            Action::ToggleHidden => "Show Hidden Files",
            Action::Rename => "Rename",
            Action::Move => "Move to Folder",
            Action::Trash => "Move to Trash",
            Action::Delete => "Delete Permanently",
            Action::Copy => "Copy",
            Action::CopyPath => "Copy Path",
            Action::Cut => "Cut",
            Action::Paste => "Paste",
            Action::Duplicate => "Duplicate",
            Action::SelectAll => "Select All",
            Action::DeselectAll => "Deselect All",
            Action::InvertSelection => "Invert Selection",
            Action::ViewFolderRules => "View Rules for This Folder",
            Action::ViewEntryRules => "View Rules for This Entry",
            Action::SaveLayoutRule => "Save This Layout as a Rule",
            Action::ShowEntry => "Always Show This Entry",
            Action::HideEntry => "Always Hide This Entry",
            Action::PickRule => "Show All Rules",
            Action::OpenTerminal => "Open Terminal Here",
            Action::Reveal => "Reveal in File Manager",
            Action::CopyName => "Copy Name",
            Action::AddPlace => "Add This Folder to the Sidebar",
            Action::MakeAlias => "Make Alias",
            Action::ToggleLayout => "List or Grid Layout",
            Action::ToggleInspector => "Show or Hide the Inspector",
            Action::ToggleFoldersFirst => "Keep Folders First",
        }
    }

    pub fn default_chord(self) -> Option<&'static str> {
        match self {
            Action::ViewFolderRules
            | Action::ViewEntryRules
            | Action::SaveLayoutRule
            | Action::ShowEntry
            | Action::HideEntry
            | Action::PickRule
            | Action::InvertSelection
            | Action::Reveal
            | Action::CopyName
            | Action::AddPlace
            | Action::MakeAlias
            | Action::ToggleLayout
            | Action::ToggleInspector
            | Action::ToggleFoldersFirst => None,
            _ => Some(self.builtin_chord()),
        }
    }

    fn builtin_chord(self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            match self {
                Action::CommandBar => "ctrl+p",
                Action::FocusSearch => "/",
                Action::OpenSettings => "super+comma",
                Action::Back => "super+bracketleft",
                Action::Forward => "super+bracketright",
                Action::EnclosingFolder => "super+up",
                Action::GoToFolder => "super+l",
                Action::Refresh => "super+r",
                Action::NewFolder => "shift+super+n",
                Action::NewFile => "alt+super+n",
                Action::ToggleHidden => "shift+super+period",
                Action::Rename => "f2",
                Action::Move => "shift+super+m",
                Action::Trash => "super+backspace",
                Action::Delete => "shift+super+backspace",
                Action::Copy => "super+c",
                Action::CopyPath => "shift+super+c",
                Action::Cut => "super+x",
                Action::Paste => "super+v",
                Action::Duplicate => "super+d",
                Action::SelectAll => "super+a",
                Action::DeselectAll => "shift+super+a",
                Action::OpenTerminal => "super+shift+t",
                _ => "",
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            match self {
                Action::CommandBar => "ctrl+p",
                Action::FocusSearch => "/",
                Action::OpenSettings => "ctrl+comma",
                Action::Back => "ctrl+bracketleft",
                Action::Forward => "ctrl+bracketright",
                Action::EnclosingFolder => "ctrl+up",
                Action::GoToFolder => "ctrl+l",
                Action::Refresh => "ctrl+r",
                Action::NewFolder => "shift+ctrl+n",
                Action::NewFile => "alt+ctrl+n",
                Action::ToggleHidden => "shift+ctrl+period",
                Action::Rename => "f2",
                Action::Move => "shift+ctrl+m",
                Action::Trash => "ctrl+backspace",
                Action::Delete => "shift+delete",
                Action::Copy => "ctrl+c",
                Action::CopyPath => "shift+ctrl+c",
                Action::Cut => "ctrl+x",
                Action::Paste => "ctrl+v",
                Action::Duplicate => "ctrl+d",
                Action::SelectAll => "ctrl+a",
                Action::DeselectAll => "shift+ctrl+a",
                Action::OpenTerminal => "ctrl+shift+t",
                _ => "",
            }
        }
    }

    pub fn icon(self, k: &Kit) -> &'static Icon {
        match self {
            Action::CommandBar => &glyphs::KEYBOARD,
            Action::FocusSearch => k.glyph.search,
            Action::OpenSettings => &theme::icons::SETTINGS_GEAR,
            Action::Back => &icons::CHEVRON_LEFT,
            Action::Forward => &icons::CHEVRON_RIGHT,
            Action::EnclosingFolder => &icons::ARROW_UP,
            Action::GoToFolder => &icons::DRIVE,
            Action::Refresh => &icons::REFRESH,
            Action::NewFolder => &icons::FOLDER_PLUS,
            Action::NewFile => &glyphs::FILE,
            Action::ToggleHidden => k.glyph.eye,
            Action::Rename => &theme::icons::EDIT,
            Action::Move => &glyphs::FOLDER,
            Action::Trash => &theme::icons::TRASH,
            Action::Delete => &theme::icons::WARNING,
            Action::Copy => &theme::icons::COPY,
            Action::CopyPath | Action::CopyName => &icons::CLIPBOARD,
            Action::Cut => &icons::SCISSORS,
            Action::Paste => &icons::CLIPBOARD,
            Action::Duplicate => &theme::icons::OUTLINE_COPY,
            Action::SelectAll => &icons::CHECK_SQUARE,
            Action::DeselectAll => &icons::SQUARE,
            Action::InvertSelection => &icons::HALF_SQUARE,
            Action::ViewFolderRules
            | Action::ViewEntryRules
            | Action::SaveLayoutRule
            | Action::PickRule => &theme::icons::EDIT,
            Action::ShowEntry | Action::HideEntry => k.glyph.eye,
            Action::OpenTerminal => &icons::TERMINAL,
            Action::Reveal => &glyphs::FOLDER_OPEN,
            Action::AddPlace => &icons::FOLDER_PLUS,
            Action::MakeAlias => &icons::LINK,
            Action::ToggleLayout => &icons::GRID,
            Action::ToggleInspector => &icons::INSPECTOR,
            Action::ToggleFoldersFirst => &icons::LIST,
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

    fn scopes(self) -> actions::Scopes {
        let typing = !matches!(
            self,
            Action::FocusSearch
                | Action::Rename
                | Action::Move
                | Action::Trash
                | Action::Delete
                | Action::Copy
                | Action::Cut
                | Action::Paste
                | Action::Duplicate
                | Action::SelectAll
                | Action::DeselectAll
                | Action::InvertSelection
        );
        let pane = matches!(
            self,
            Action::CommandBar | Action::OpenSettings | Action::OpenTerminal
        );
        actions::Scopes {
            while_typing: typing,
            in_pane: pane,
            in_modal: true,
        }
    }

    fn in_palette(self) -> bool {
        self != Action::CommandBar
    }

    fn ignores_shift(self) -> bool {
        self == Action::CommandBar
    }
}
