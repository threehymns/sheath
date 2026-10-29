//! The app's keyboard shortcuts: the chord that runs each command, matching a key event to it,
//! and the KDL file they're saved in. Keybinds have their own file; anything else the app saves
//! later gets a file of its own rather than sharing this one.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::config;
use crate::recorder::Recorder;
use rune::{Key, KeyEvent, PhysicalKeyCode, Shortcut, ShortcutError};

/// The file name under the config folder. Only keybinds live in it.
const FILE: &str = "keybinds.kdl";

/// A command a shortcut can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    CommandBar,
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
}

impl Action {
    /// Every action, in the order the settings dialog lists them.
    pub const ALL: [Action; 15] = [
        Action::CommandBar,
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
    ];

    /// Whether Shift is ignored when matching this action, as editor command bars treat
    /// Ctrl+P and Ctrl+Shift+P alike.
    pub fn ignores_shift(self) -> bool {
        self == Action::CommandBar
    }

    /// The stable name the file and the element keys use.
    pub fn id(self) -> &'static str {
        match self {
            Action::CommandBar => "command-bar",
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
        }
    }

    /// What the settings dialog calls it.
    pub fn label(self) -> &'static str {
        match self {
            Action::CommandBar => "Command Bar",
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
        }
    }

    fn from_id(id: &str) -> Option<Action> {
        Action::ALL.iter().copied().find(|action| action.id() == id)
    }

    /// The chord a fresh install has, as the file stores it. The command bar answers to Ctrl+P
    /// on every platform, shifted or not: [`Action::ignores_shift`].
    fn default_chord(self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            match self {
                Action::CommandBar => "ctrl+p",
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
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            match self {
                Action::CommandBar => "ctrl+p",
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
            }
        }
    }
}

/// The app's keybindings, and the file they came from.
pub struct Keybinds {
    chords: BTreeMap<Action, Shortcut>,
    path: Option<PathBuf>,
    /// Why the file couldn't be read or written, for the settings dialog.
    pub error: Option<String>,
}

impl Keybinds {
    /// The built-in bindings, as if no file existed. They have nowhere to save, so changing
    /// them needs [`Keybinds::at`] or [`Keybinds::load`].
    pub fn defaults() -> Keybinds {
        Keybinds {
            chords: default_chords(),
            path: None,
            error: None,
        }
    }

    /// The bindings saved for this user, or the defaults when there's no readable file. A file
    /// with a problem leaves the defaults in place and reports it through [`Keybinds::error`].
    pub fn load() -> Keybinds {
        match config::path(FILE) {
            Some(path) => Keybinds::at(path),
            None => {
                let mut binds = Keybinds::defaults();
                binds.error = Some("Couldn’t find a folder to save keybinds in.".into());
                binds
            }
        }
    }

    /// The bindings in `path`, for a config directory chosen by the caller.
    pub fn at(path: PathBuf) -> Keybinds {
        let mut binds = Keybinds {
            chords: default_chords(),
            path: Some(path.clone()),
            error: None,
        };
        match fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(chords) => {
                    for (action, chord) in chords {
                        match chord {
                            Some(chord) => {
                                binds.chords.insert(action, chord);
                            }
                            None => {
                                binds.chords.remove(&action);
                            }
                        }
                    }
                }
                Err(err) => {
                    binds.error = Some(format!(
                        "Couldn’t read {}: {err}. Using the defaults.",
                        path.display()
                    ));
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                binds.error = Some(format!(
                    "Couldn’t read {}: {err}. Using the defaults.",
                    path.display()
                ));
            }
        }
        binds
    }

    /// The file keybinds are saved in, when there is one.
    pub fn file() -> Option<PathBuf> {
        config::path(FILE)
    }

    pub fn chord(&self, action: Action) -> Option<&Shortcut> {
        self.chords.get(&action)
    }

    /// One fresh recorder per action, nothing listened for yet: what the settings dialog edits.
    /// The bound chords stay in [`Keybinds`], where the recorder's field reads them.
    pub fn recorders() -> BTreeMap<Action, Recorder> {
        Action::ALL
            .into_iter()
            .map(|action| (action, Recorder::default()))
            .collect()
    }

    /// The chord as the kit wants it for a button's hint (`"super+r"`), or empty when unbound.
    pub fn accelerator(&self, action: Action) -> String {
        self.chord(action)
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    /// The chord's keycaps (`["⌘", "R"]`, `["Ctrl", "R"]`) for drawing as a [`Keybind`], or
    /// empty when unbound.
    ///
    /// [`Keybind`]: rune_kit::Keybind
    pub fn keycaps(&self, action: Action) -> Vec<String> {
        self.chord(action)
            .map(Shortcut::menu_keycaps)
            .unwrap_or_default()
    }

    /// The chord as a menu shows it (`"⌘R"`, `"Ctrl+R"`), or empty when unbound.
    pub fn menu_label(&self, action: Action) -> String {
        let separator = if cfg!(target_os = "macos") { "" } else { "+" };
        self.keycaps(action).join(separator)
    }

    /// Whether `event` is the chord bound to `action`, or, for an action that ignores Shift,
    /// the same chord with Shift held.
    pub fn matches(&self, action: Action, event: &KeyEvent) -> bool {
        self.chord(action)
            .is_some_and(|chord| matches(chord, event, action.ignores_shift()))
    }

    /// Binds `action` to `shortcut`, or clears it, and saves. A chord another action already
    /// uses is refused with [`ShortcutError::Conflict`]; the recorder shows that on the field.
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
        self.save();
        Ok(())
    }

    /// Whether the file names `action`, so its shortcut is one the user set rather than the
    /// built-in one. Only those have a Reset: resetting takes the line out of the file, and a
    /// file that doesn't name an action already gets its default.
    pub fn is_custom(&self, action: Action) -> bool {
        self.chords.get(&action) != default_shortcut(action).as_ref()
    }

    /// Takes `action` out of the file, so the built-in chord applies again, and saves.
    pub fn reset(&mut self, action: Action) {
        match default_shortcut(action) {
            Some(chord) => self.chords.insert(action, chord),
            None => self.chords.remove(&action),
        };
        self.save();
    }

    /// Writes the bindings. A failure keeps the change for this session and reports it, so the
    /// settings dialog can say why the next launch will differ.
    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if let Some(dir) = path.parent()
            && let Err(err) = fs::create_dir_all(dir)
        {
            self.error = Some(format!("Couldn’t make {}: {err}", dir.display()));
            return;
        }
        match fs::write(&path, render(&self.chords)) {
            Ok(()) => self.error = None,
            Err(err) => self.error = Some(format!("Couldn’t save {}: {err}", path.display())),
        }
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

/// The chord a fresh install has for `action`.
fn default_shortcut(action: Action) -> Option<Shortcut> {
    let text = action.default_chord();
    match text.parse::<Shortcut>() {
        Ok(chord) => Some(chord),
        // A typo in a default is a programming error, but shipping a dead app is worse.
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

/// The KDL file: a `keybinds` node whose children are `action "chord"` for the shortcuts that
/// differ from the built-in ones; an empty string means the action has no shortcut. Actions the
/// file leaves out keep their defaults, so resetting one is deleting its line.
fn render(chords: &BTreeMap<Action, Shortcut>) -> String {
    let mut doc = KdlDocument::new();
    let mut node = KdlNode::new("keybinds");
    let mut children = KdlDocument::new();
    for action in Action::ALL {
        let chord = chords.get(&action);
        if chord == default_shortcut(action).as_ref() {
            continue;
        }
        let mut child = KdlNode::new(action.id());
        child.push(KdlEntry::new(
            chord.map(ToString::to_string).unwrap_or_default(),
        ));
        children.nodes_mut().push(child);
    }
    node.set_children(children);
    doc.nodes_mut().push(node);
    // The file is meant to be read and edited by hand, so indent it.
    doc.autoformat();
    format!(
        "// Sheath keyboard shortcuts. Only keybinds live in this file; other settings get their\n\
         // own. It lists the shortcuts that differ from the built-in ones: an empty chord clears\n\
         // one, and deleting a line gives that action its default back.\n\
         {doc}"
    )
}

/// The actions the file names, with `None` for a cleared chord. Unknown actions and unreadable
/// chords are skipped, so an edited file degrades instead of breaking the app.
fn parse(text: &str) -> Result<Vec<(Action, Option<Shortcut>)>, String> {
    let doc: KdlDocument = text.parse().map_err(|err: kdl::KdlError| err.to_string())?;
    let mut out = Vec::new();
    for node in doc.nodes() {
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
    Ok(out)
}

/// Whether `event` is `chord`. Rune matches window accelerators by the key's position, so the
/// logical key an event carries is mapped back to the physical one the chord stores.
fn matches(chord: &Shortcut, event: &KeyEvent, ignore_shift: bool) -> bool {
    let want = chord.modifiers();
    let held = event.mods;
    // On macOS the Command key reports both `cmd` and `logo`, and a chord's Super bit is the
    // same key; elsewhere Super is a separate key that must agree.
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
