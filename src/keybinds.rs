use rune::Icon;
use rune_kit::{Kit, glyphs};
use rune_theme_opencode as theme;

use crate::icons;

// The commands, their ids, labels and per-OS chords live in one declaration: adding an action
// here adds it to the palette, the keybind editor and the defaults. What the command does is the
// exhaustive `perform` match in `app/commands.rs`; what it looks like is `icon` below.
actions::declare_actions! {
    /// The app's commands. The enum stays here so `perform` is an exhaustive match.
    pub enum Action {
        CommandBar = "command-bar", "Command Bar", chord("ctrl+p", "ctrl+p"),
        FocusSearch = "focus-search", "Search This Folder", chord("/", "/"),
        OpenSettings = "open-settings", "Settings", chord("ctrl+comma", "super+comma"),
        Back = "back", "Back", chord("ctrl+bracketleft", "super+bracketleft"),
        Forward = "forward", "Forward", chord("ctrl+bracketright", "super+bracketright"),
        EnclosingFolder = "enclosing-folder", "Enclosing Folder", chord("ctrl+up", "super+up"),
        Refresh = "refresh", "Refresh", chord("ctrl+r", "super+r"),
        NewFolder = "new-folder", "New Folder", chord("shift+ctrl+n", "shift+super+n"),
        ToggleHidden = "toggle-hidden", "Show Hidden Files", chord("shift+ctrl+period", "shift+super+period"),
        Rename = "rename", "Rename", chord("f2", "f2"),
        Move = "move", "Move to Folder", chord("shift+ctrl+m", "shift+super+m"),
        Trash = "trash", "Move to Trash", chord("ctrl+backspace", "super+backspace"),
        Delete = "delete", "Delete Permanently", chord("shift+delete", "shift+super+backspace"),
        Copy = "copy", "Copy", chord("ctrl+c", "super+c"),
        Cut = "cut", "Cut", chord("ctrl+x", "super+x"),
        Paste = "paste", "Paste", chord("ctrl+v", "super+v"),
        Duplicate = "duplicate", "Duplicate", chord("ctrl+d", "super+d"),
        SelectAll = "select-all", "Select All", chord("ctrl+a", "super+a"),
        DeselectAll = "deselect-all", "Deselect All", chord("shift+ctrl+a", "shift+super+a"),
        InvertSelection = "invert-selection", "Invert Selection", unbound,
        GoToFolder = "go-to-folder", "Go to Folder…", chord("ctrl+l", "super+l"),
        NewFile = "new-file", "New File", chord("alt+ctrl+n", "alt+super+n"),
        CopyPath = "copy-path", "Copy Path", chord("shift+ctrl+c", "shift+super+c"),
        ViewFolderRules = "view-folder-rules", "View Rules for This Folder", unbound,
        ViewEntryRules = "view-entry-rules", "View Rules for This Entry", unbound,
        SaveLayoutRule = "save-layout-rule", "Save This Layout as a Rule", unbound,
        ShowEntry = "show-entry", "Always Show This Entry", unbound,
        HideEntry = "hide-entry", "Always Hide This Entry", unbound,
        PickRule = "pick-rule", "Show All Rules", unbound,
        OpenTerminal = "open-terminal", "Open Terminal Here", chord("ctrl+shift+t", "super+shift+t"),
        Reveal = "reveal", "Reveal in File Manager", unbound,
        CopyName = "copy-name", "Copy Name", unbound,
        AddPlace = "add-place", "Add This Folder to the Sidebar", unbound,
        MakeAlias = "make-alias", "Make Alias", unbound,
        ToggleLayout = "toggle-layout", "List or Grid Layout", unbound,
        ToggleInspector = "toggle-inspector", "Show or Hide the Inspector", unbound,
        ToggleFoldersFirst = "toggle-folders-first", "Keep Folders First", unbound,
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

/// The shared keymap, with this app's action type.
pub type Keybinds = actions::Keybinds<Action>;

impl Action {
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
