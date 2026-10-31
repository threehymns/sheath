use std::collections::BTreeMap;

use kdl::KdlDocument;

use crate::recorder::Recorder;
use rune::{Key, KeyEvent, PhysicalKeyCode, Shortcut, ShortcutError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    CommandBar,
    FocusSearch,
    OpenSettings,
    Back,
    Forward,
    EnclosingFolder,
    Refresh,
    NewFolder,
    ToggleHidden,
    Rename,
    Move,
    Trash,
    Copy,
    Cut,
    Paste,
    Duplicate,
    ViewFolderRules,
    ViewEntryRules,
    SaveLayoutRule,
    ShowEntry,
    HideEntry,
    PickRule,
}

impl Action {
    pub const ALL: [Action; 22] = [
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
        Action::Copy,
        Action::Cut,
        Action::Paste,
        Action::Duplicate,
        Action::ViewFolderRules,
        Action::ViewEntryRules,
        Action::SaveLayoutRule,
        Action::ShowEntry,
        Action::HideEntry,
        Action::PickRule,
    ];

    pub fn ignores_shift(self) -> bool {
        self == Action::CommandBar
    }

    pub fn id(self) -> &'static str {
        match self {
            Action::CommandBar => "command-bar",
            Action::FocusSearch => "focus-search",
            Action::OpenSettings => "open-settings",
            Action::Back => "back",
            Action::Forward => "forward",
            Action::EnclosingFolder => "enclosing-folder",
            Action::Refresh => "refresh",
            Action::NewFolder => "new-folder",
            Action::ToggleHidden => "toggle-hidden",
            Action::Rename => "rename",
            Action::Move => "move",
            Action::Trash => "trash",
            Action::Copy => "copy",
            Action::Cut => "cut",
            Action::Paste => "paste",
            Action::Duplicate => "duplicate",
            Action::ViewFolderRules => "view-folder-rules",
            Action::ViewEntryRules => "view-entry-rules",
            Action::SaveLayoutRule => "save-layout-rule",
            Action::ShowEntry => "show-entry",
            Action::HideEntry => "hide-entry",
            Action::PickRule => "pick-rule",
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
            Action::Refresh => "Refresh",
            Action::NewFolder => "New Folder",
            Action::ToggleHidden => "Show Hidden Files",
            Action::Rename => "Rename",
            Action::Move => "Move to Folder",
            Action::Trash => "Move to Trash",
            Action::Copy => "Copy",
            Action::Cut => "Cut",
            Action::Paste => "Paste",
            Action::Duplicate => "Duplicate",
            Action::ViewFolderRules => "View Rules for This Folder",
            Action::ViewEntryRules => "View Rules for This Entry",
            Action::SaveLayoutRule => "Save This Layout as a Rule",
            Action::ShowEntry => "Always Show This Entry",
            Action::HideEntry => "Always Hide This Entry",
            Action::PickRule => "Show All Rules",
        }
    }

    fn from_id(id: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|action| action.id() == id)
    }

    fn default_chord(self) -> Option<&'static str> {
        match self {
            Action::ViewFolderRules
            | Action::ViewEntryRules
            | Action::SaveLayoutRule
            | Action::ShowEntry
            | Action::HideEntry
            | Action::PickRule => None,
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
                Action::Refresh => "super+r",
                Action::NewFolder => "shift+super+n",
                Action::ToggleHidden => "shift+super+period",
                Action::Rename => "f2",
                Action::Move => "shift+super+m",
                Action::Trash => "super+backspace",
                Action::Copy => "super+c",
                Action::Cut => "super+x",
                Action::Paste => "super+v",
                Action::Duplicate => "super+d",
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
                Action::Refresh => "ctrl+r",
                Action::NewFolder => "shift+ctrl+n",
                Action::ToggleHidden => "shift+ctrl+period",
                Action::Rename => "f2",
                Action::Move => "shift+ctrl+m",
                Action::Trash => "ctrl+backspace",
                Action::Copy => "ctrl+c",
                Action::Cut => "ctrl+x",
                Action::Paste => "ctrl+v",
                Action::Duplicate => "ctrl+d",
                _ => "",
            }
        }
    }
}

pub struct Keybinds {
    chords: BTreeMap<Action, Shortcut>,
}

impl Keybinds {
    pub fn defaults() -> Keybinds {
        Keybinds {
            chords: default_chords(),
        }
    }

    pub fn from_doc(doc: Option<&KdlDocument>) -> Keybinds {
        let mut binds = Keybinds::defaults();
        let Some(doc) = doc else {
            return binds;
        };
        for (action, chord) in parse(doc) {
            match chord {
                Some(chord) => {
                    binds.chords.insert(action, chord);
                }
                None => {
                    binds.chords.remove(&action);
                }
            }
        }
        binds
    }

    pub fn write(&self, doc: &mut KdlDocument, action: Action) {
        let current = self.chords.get(&action);
        if current == default_shortcut(action).as_ref() {
            crate::settings::remove_child(doc, "keybinds", action.id());
            return;
        }
        let text = match current {
            Some(chord) => format!("{} \"{chord}\"", action.id()),
            None => format!("{} \"\"", action.id()),
        };
        crate::settings::write_child(doc, "keybinds", action.id(), &text);
    }

    pub fn chord(&self, action: Action) -> Option<&Shortcut> {
        self.chords.get(&action)
    }

    pub fn recorders() -> BTreeMap<Action, Recorder> {
        Action::ALL
            .into_iter()
            .map(|action| (action, Recorder::default()))
            .collect()
    }

    pub fn accelerator(&self, action: Action) -> String {
        self.chord(action)
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    pub fn keycaps(&self, action: Action) -> Vec<String> {
        self.chord(action)
            .map(Shortcut::menu_keycaps)
            .unwrap_or_default()
    }

    pub fn menu_label(&self, action: Action) -> String {
        let separator = if cfg!(target_os = "macos") { "" } else { "+" };
        self.keycaps(action).join(separator)
    }

    pub fn matches(&self, action: Action, event: &KeyEvent) -> bool {
        self.chord(action)
            .is_some_and(|chord| matches(chord, event, action.ignores_shift()))
    }

    pub fn set(
        &mut self,
        action: Action,
        shortcut: Option<&Shortcut>,
    ) -> Result<(), ShortcutError> {
        if let Some(chord) = shortcut
            && self
                .chords
                .iter()
                .any(|(other, bound)| *other != action && bound == chord)
        {
            return Err(ShortcutError::Conflict);
        }
        match shortcut {
            Some(chord) => {
                self.chords.insert(action, chord.clone());
            }
            None => {
                self.chords.remove(&action);
            }
        }
        Ok(())
    }

    pub fn is_custom(&self, action: Action) -> bool {
        self.chords.get(&action) != default_shortcut(action).as_ref()
    }

    pub fn reset(&mut self, action: Action) {
        match default_shortcut(action) {
            Some(chord) => self.chords.insert(action, chord),
            None => self.chords.remove(&action),
        };
    }
}

fn default_chords() -> BTreeMap<Action, Shortcut> {
    let mut chords = BTreeMap::new();
    for action in Action::ALL {
        if let Some(chord) = default_shortcut(action) {
            chords.insert(action, chord);
        }
    }
    chords
}

fn default_shortcut(action: Action) -> Option<Shortcut> {
    let text = action.default_chord()?;
    match text.parse::<Shortcut>() {
        Ok(chord) => Some(chord),
        Err(_) => {
            debug_assert!(
                false,
                "{} has an invalid default chord: {text}",
                action.id()
            );
            None
        }
    }
}

fn parse(doc: &KdlDocument) -> Vec<(Action, Option<Shortcut>)> {
    let mut out = Vec::new();
    for node in doc.nodes() {
        if node.name().value() != "keybinds" {
            continue;
        }
        for child in node.children().into_iter().flat_map(|kids| kids.nodes()) {
            let Some(action) = Action::from_id(child.name().value()) else {
                continue;
            };
            let raw = child
                .get(0)
                .and_then(|value| value.as_string())
                .unwrap_or_default()
                .trim();
            let chord = if raw.is_empty() {
                None
            } else {
                match raw.parse::<Shortcut>() {
                    Ok(chord) => Some(chord),
                    Err(_) => continue,
                }
            };
            out.push((action, chord));
        }
    }
    out
}

fn matches(chord: &Shortcut, event: &KeyEvent, ignore_shift: bool) -> bool {
    let want = chord.modifiers();
    let held = event.mods;
    let logo = cfg!(target_os = "macos") || want.logo == held.logo;
    if (want.shift != held.shift && !ignore_shift)
        || want.alt != held.alt
        || want.cmd != held.cmd
        || want.ctrl != held.ctrl
        || !logo
    {
        return false;
    }
    physical(&event.key) == Some(chord.physical_key())
}

// Rune matches accelerators by physical position, so the logical key an event carries is mapped
// back to it.
fn physical(key: &Key) -> Option<PhysicalKeyCode> {
    use PhysicalKeyCode as Code;
    Some(match key {
        Key::Command(c) => char_key(*c)?,
        Key::Text(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            char_key(c)?
        }
        Key::Enter => Code::Enter,
        Key::Backspace => Code::Backspace,
        Key::Delete => Code::Delete,
        Key::Left => Code::ArrowLeft,
        Key::Right => Code::ArrowRight,
        Key::Up => Code::ArrowUp,
        Key::Down => Code::ArrowDown,
        Key::Home => Code::Home,
        Key::End => Code::End,
        Key::PageUp => Code::PageUp,
        Key::PageDown => Code::PageDown,
        Key::Escape => Code::Escape,
        Key::Tab => Code::Tab,
        Key::Function(n) => function_key(*n)?,
        _ => return None,
    })
}

fn char_key(c: char) -> Option<PhysicalKeyCode> {
    use PhysicalKeyCode as Code;
    Some(match c.to_ascii_lowercase() {
        'a' => Code::KeyA,
        'b' => Code::KeyB,
        'c' => Code::KeyC,
        'd' => Code::KeyD,
        'e' => Code::KeyE,
        'f' => Code::KeyF,
        'g' => Code::KeyG,
        'h' => Code::KeyH,
        'i' => Code::KeyI,
        'j' => Code::KeyJ,
        'k' => Code::KeyK,
        'l' => Code::KeyL,
        'm' => Code::KeyM,
        'n' => Code::KeyN,
        'o' => Code::KeyO,
        'p' => Code::KeyP,
        'q' => Code::KeyQ,
        'r' => Code::KeyR,
        's' => Code::KeyS,
        't' => Code::KeyT,
        'u' => Code::KeyU,
        'v' => Code::KeyV,
        'w' => Code::KeyW,
        'x' => Code::KeyX,
        'y' => Code::KeyY,
        'z' => Code::KeyZ,
        '0' => Code::Digit0,
        '1' => Code::Digit1,
        '2' => Code::Digit2,
        '3' => Code::Digit3,
        '4' => Code::Digit4,
        '5' => Code::Digit5,
        '6' => Code::Digit6,
        '7' => Code::Digit7,
        '8' => Code::Digit8,
        '9' => Code::Digit9,
        '-' => Code::Minus,
        '=' => Code::Equal,
        ',' => Code::Comma,
        '.' => Code::Period,
        '/' => Code::Slash,
        ';' => Code::Semicolon,
        '\'' => Code::Quote,
        '[' => Code::BracketLeft,
        ']' => Code::BracketRight,
        '\\' => Code::Backslash,
        '`' => Code::Backquote,
        ' ' => Code::Space,
        _ => return None,
    })
}

fn function_key(n: u8) -> Option<PhysicalKeyCode> {
    use PhysicalKeyCode as Code;
    Some(match n {
        1 => Code::F1,
        2 => Code::F2,
        3 => Code::F3,
        4 => Code::F4,
        5 => Code::F5,
        6 => Code::F6,
        7 => Code::F7,
        8 => Code::F8,
        9 => Code::F9,
        10 => Code::F10,
        11 => Code::F11,
        12 => Code::F12,
        13 => Code::F13,
        14 => Code::F14,
        15 => Code::F15,
        16 => Code::F16,
        17 => Code::F17,
        18 => Code::F18,
        19 => Code::F19,
        20 => Code::F20,
        21 => Code::F21,
        22 => Code::F22,
        23 => Code::F23,
        24 => Code::F24,
        _ => return None,
    })
}
