//! What the user does, encoded for the program: keys under the modes it set, the mouse when it
//! asked for it, and pastes wrapped the way it asked for. Every path writes to the PTY and lets
//! libghostty own the meaning.

use ghostty_vt::key;
use ghostty_vt::mouse;
use ghostty_vt::terminal::Mode;
use rune::{Key, KeyEvent};

use super::Engine;

impl Engine {
    /// Encodes a key with the program's current modes and writes it, reusing one encoder event
    /// and one buffer: a keypress is a round trip through the kernel, so it should not also cost
    /// two allocations. The pane consumes the key either way.
    pub fn send_key(&mut self, event: &KeyEvent) -> bool {
        if matches!(event.key, Key::Modifier { .. }) {
            return true;
        }
        let Some(key) = glyph(&event.key) else {
            return false;
        };
        let encoded = self
            .key_event
            .set_action(key::Action::Press)
            .set_key(key)
            .set_mods(mods(event));
        match &event.key {
            Key::Text(text) => {
                encoded.set_utf8(Some(text.clone()));
            }
            _ => {
                encoded.set_utf8(None::<String>);
            }
        }
        if self.keys_stale {
            // Modes the program set (DECCKM, the kitty keyboard protocol) only change with
            // output, so the encoder reads them once per change rather than once per key.
            self.keys.set_options_from_terminal(&self.vt);
            self.keys_stale = false;
        }
        self.keybuf.clear();
        if self
            .keys
            .encode_to_vec(&self.key_event, &mut self.keybuf)
            .is_err()
        {
            self.keybuf.clear();
        }
        if !self.keybuf.is_empty() {
            let _ = self.writer.write_all(&self.keybuf);
            let _ = self.writer.flush();
        }
        true
    }

    pub fn mouse(
        &mut self,
        action: mouse::Action,
        button: Option<mouse::Button>,
        at: (u16, u16),
        shift: bool,
    ) -> Vec<u8> {
        let Ok(mut event) = mouse::Event::new() else {
            return Vec::new();
        };
        event
            .set_action(action)
            .set_button(button)
            .set_position(mouse::Position {
                x: at.0 as f32,
                y: at.1 as f32,
            });
        if shift {
            event.set_mods(key::Mods::SHIFT);
        }
        self.mouse.set_options_from_terminal(&self.vt);
        let mut out = Vec::new();
        if self.mouse.encode_to_vec(&event, &mut out).is_ok() {
            out
        } else {
            Vec::new()
        }
    }

    pub fn paste(&mut self, text: &str) {
        let bracketed = self.vt.mode(Mode::BRACKETED_PASTE).unwrap_or(false);
        let mut data = text.as_bytes().to_vec();
        let mut out = vec![0u8; data.len() + 16];
        match ghostty_vt::paste::encode(&mut data, bracketed, &mut out) {
            Ok(len) => self.write(&out[..len]),
            Err(ghostty_vt::Error::OutOfSpace { required }) => {
                let mut out = vec![0u8; required];
                if let Ok(len) = ghostty_vt::paste::encode(&mut data, bracketed, &mut out) {
                    self.write(&out[..len]);
                }
            }
            Err(_) => {}
        }
    }
}

fn mods(event: &KeyEvent) -> key::Mods {
    let held = event.mods;
    let mut out = key::Mods::empty();
    if held.shift {
        out |= key::Mods::SHIFT;
    }
    if held.alt {
        out |= key::Mods::ALT;
    }
    if held.ctrl {
        out |= key::Mods::CTRL;
    }
    let logo = if cfg!(target_os = "macos") {
        held.cmd || held.logo
    } else {
        held.logo
    };
    if logo {
        out |= key::Mods::SUPER;
    }
    out
}

fn glyph(key: &Key) -> Option<key::Key> {
    use key::Key as G;
    Some(match key {
        Key::Text(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                // Multi-character text carries its content through `utf8` alone.
                return Some(G::Unidentified);
            }
            char_glyph(c)
        }
        Key::Command(c) => char_glyph(*c),
        Key::Enter => G::Enter,
        Key::Backspace => G::Backspace,
        Key::Delete => G::Delete,
        Key::Left => G::ArrowLeft,
        Key::Right => G::ArrowRight,
        Key::Up => G::ArrowUp,
        Key::Down => G::ArrowDown,
        Key::Home => G::Home,
        Key::End => G::End,
        Key::PageUp => G::PageUp,
        Key::PageDown => G::PageDown,
        Key::Escape => G::Escape,
        Key::Tab => G::Tab,
        Key::Function(n) => match n {
            1 => G::F1,
            2 => G::F2,
            3 => G::F3,
            4 => G::F4,
            5 => G::F5,
            6 => G::F6,
            7 => G::F7,
            8 => G::F8,
            9 => G::F9,
            10 => G::F10,
            11 => G::F11,
            12 => G::F12,
            _ => return None,
        },
        _ => return None,
    })
}

fn char_glyph(c: char) -> key::Key {
    use key::Key as G;
    match c.to_ascii_lowercase() {
        'a' => G::A,
        'b' => G::B,
        'c' => G::C,
        'd' => G::D,
        'e' => G::E,
        'f' => G::F,
        'g' => G::G,
        'h' => G::H,
        'i' => G::I,
        'j' => G::J,
        'k' => G::K,
        'l' => G::L,
        'm' => G::M,
        'n' => G::N,
        'o' => G::O,
        'p' => G::P,
        'q' => G::Q,
        'r' => G::R,
        's' => G::S,
        't' => G::T,
        'u' => G::U,
        'v' => G::V,
        'w' => G::W,
        'x' => G::X,
        'y' => G::Y,
        'z' => G::Z,
        '0' => G::Digit0,
        '1' => G::Digit1,
        '2' => G::Digit2,
        '3' => G::Digit3,
        '4' => G::Digit4,
        '5' => G::Digit5,
        '6' => G::Digit6,
        '7' => G::Digit7,
        '8' => G::Digit8,
        '9' => G::Digit9,
        ' ' => G::Space,
        '-' => G::Minus,
        '=' => G::Equal,
        ',' => G::Comma,
        '.' => G::Period,
        '/' => G::Slash,
        ';' => G::Semicolon,
        '\'' => G::Quote,
        '[' => G::BracketLeft,
        ']' => G::BracketRight,
        '\\' => G::Backslash,
        '`' => G::Backquote,
        _ => G::Unidentified,
    }
}
