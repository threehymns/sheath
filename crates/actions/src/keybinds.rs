use std::collections::BTreeMap;

use rune::{Key, KeyEvent, PhysicalKeyCode, Shortcut, ShortcutError};

use crate::action::Action;
use crate::recorder::Recorder;

/// The chords an app's actions resolve to. The app defines the action type; this owns
/// defaults, conflicts, physical-key matching and the labels a menu or palette shows.
pub struct Keybinds<A: Action> {
    chords: BTreeMap<A, Shortcut>,
}

impl<A: Action> Keybinds<A> {
    pub fn defaults() -> Keybinds<A> {
        Keybinds {
            chords: default_chords(),
        }
    }

    /// The defaults with `entries` applied: a chord overrides, `None` clears.
    pub fn from_entries(entries: impl IntoIterator<Item = (A, Option<Shortcut>)>) -> Keybinds<A> {
        let mut binds = Keybinds::defaults();
        for (action, chord) in entries {
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

    pub fn chord(&self, action: A) -> Option<&Shortcut> {
        self.chords.get(&action)
    }

    pub fn recorders() -> BTreeMap<A, Recorder> {
        A::ALL
            .iter()
            .copied()
            .map(|action| (action, Recorder::default()))
            .collect()
    }

    pub fn accelerator(&self, action: A) -> String {
        self.chord(action)
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    pub fn keycaps(&self, action: A) -> Vec<String> {
        self.chord(action)
            .map(Shortcut::menu_keycaps)
            .unwrap_or_default()
    }

    pub fn menu_label(&self, action: A) -> String {
        let separator = if cfg!(target_os = "macos") { "" } else { "+" };
        self.keycaps(action).join(separator)
    }

    pub fn matches(&self, action: A, event: &KeyEvent) -> bool {
        self.chord(action)
            .is_some_and(|chord| matches(chord, event, action.ignores_shift()))
    }

    pub fn set(&mut self, action: A, shortcut: Option<&Shortcut>) -> Result<(), ShortcutError> {
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

    pub fn is_custom(&self, action: A) -> bool {
        self.chords.get(&action) != default_shortcut(action).as_ref()
    }

    pub fn reset(&mut self, action: A) {
        match default_shortcut(action) {
            Some(chord) => self.chords.insert(action, chord),
            None => self.chords.remove(&action),
        };
    }
}

pub fn default_shortcut<A: Action>(action: A) -> Option<Shortcut> {
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

fn default_chords<A: Action>() -> BTreeMap<A, Shortcut> {
    let mut chords = BTreeMap::new();
    for action in A::ALL.iter().copied() {
        if let Some(chord) = default_shortcut(action) {
            chords.insert(action, chord);
        }
    }
    chords
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
