//! What the settings keybinds are listening for: the action's recorder, the modifiers held so
//! far, and the chord and error a refused change left.

use rune::{Mods, PhysicalInput, PhysicalKeyCode, Shortcut, ShortcutError, modifier_keycaps};

/// What the recorder made of one physical input.
pub enum Outcome {
    /// Not listening: the event belongs to whatever else has it.
    Pass,
    /// The recorder took the event.
    Consumed,
    /// The user played a chord: bind it, or clear the action when it is `None`.
    Change(Option<Shortcut>),
}

/// One action's recording state, between the click that starts it and the chord that ends it.
#[derive(Default)]
pub struct Recorder {
    recording: bool,
    held: Mods,
    error: Option<ShortcutError>,
    /// The chord the error refused, shown with it until a new chord starts. None for a refused
    /// clear.
    refused: Option<Shortcut>,
    /// Swallow auto-repeat until the captured or cancelled key is released.
    finishing: Option<PhysicalKeyCode>,
}

impl Recorder {
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn error(&self) -> Option<&ShortcutError> {
        self.error.as_ref()
    }

    /// The chord `finish` just refused, until another chord starts.
    pub fn refused(&self) -> Option<&Shortcut> {
        self.refused.as_ref()
    }

    /// Starts listening, dropping the error the last chord left.
    pub fn begin(&mut self) {
        self.recording = true;
        self.error = None;
        self.refused = None;
        self.finishing = None;
    }

    /// Stops listening without binding anything.
    pub fn cancel(&mut self) {
        self.recording = false;
        self.held = Mods::default();
        self.error = None;
        self.refused = None;
    }

    /// The keys the field shows: the refused chord, the modifiers held so far, or the chord the
    /// action is bound to.
    pub fn keycaps(&self, bound: Option<&Shortcut>) -> Vec<String> {
        if let Some(refused) = self.refused.as_ref().filter(|_| self.recording) {
            refused.keycaps()
        } else if self.recording {
            modifier_keycaps(self.held)
        } else {
            bound.map(Shortcut::keycaps).unwrap_or_default()
        }
    }

    /// Takes one physical input. Escape cancels, Backspace clears, and any other key proposes
    /// the chord it makes with the modifiers held.
    pub fn input(&mut self, event: &PhysicalInput) -> Outcome {
        match *event {
            PhysicalInput::FocusLost => {
                let active = self.recording || self.finishing.is_some();
                self.cancel();
                self.finishing = None;
                if active {
                    Outcome::Consumed
                } else {
                    Outcome::Pass
                }
            }
            PhysicalInput::Modifiers(mods) => {
                self.hold(mods);
                if self.recording {
                    Outcome::Consumed
                } else {
                    Outcome::Pass
                }
            }
            PhysicalInput::Key {
                key,
                mods,
                pressed,
                repeat,
            } => {
                self.hold(mods);
                if self.finishing == Some(key) {
                    if !pressed {
                        self.finishing = None;
                    }
                    return Outcome::Consumed;
                }
                if !self.recording {
                    return Outcome::Pass;
                }
                if !pressed || repeat || is_modifier(key) {
                    return Outcome::Consumed;
                }
                self.finishing = Some(key);
                if key == PhysicalKeyCode::Escape {
                    self.cancel();
                    return Outcome::Consumed;
                }
                if key == PhysicalKeyCode::Backspace {
                    return Outcome::Change(None);
                }
                match Shortcut::from_physical_key(key, mods) {
                    Ok(shortcut) => Outcome::Change(Some(shortcut)),
                    Err(error) => {
                        self.error = Some(error);
                        self.refused = None;
                        Outcome::Consumed
                    }
                }
            }
        }
    }

    /// Applies the answer to the chord `input` proposed: listening ends when it bound, and the
    /// error stays on screen with the refused chord when it didn't.
    pub fn finish(&mut self, next: Option<Shortcut>, result: Result<(), ShortcutError>) {
        match result {
            Ok(()) => self.cancel(),
            Err(error) => {
                self.error = Some(error);
                self.refused = next;
            }
        }
    }

    /// Pressing a new modifier starts another chord: the refused one and its error go.
    fn hold(&mut self, mods: Mods) {
        let held = self.held;
        if (mods.ctrl && !held.ctrl)
            || (mods.alt && !held.alt)
            || (mods.shift && !held.shift)
            || (mods.cmd && !held.cmd)
            || (mods.logo && !held.logo)
        {
            self.refused = None;
            self.error = None;
        }
        self.held = mods;
    }
}

fn is_modifier(key: PhysicalKeyCode) -> bool {
    use PhysicalKeyCode::*;
    matches!(
        key,
        ShiftLeft
            | ShiftRight
            | ControlLeft
            | ControlRight
            | AltLeft
            | AltRight
            | SuperLeft
            | SuperRight
            | Fn
            | FnLock
    )
}
