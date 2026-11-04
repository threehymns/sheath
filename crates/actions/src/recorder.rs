use rune::{Mods, PhysicalInput, PhysicalKeyCode, Shortcut, ShortcutError, modifier_keycaps};

pub enum Outcome {
    Pass,
    Consumed,
    Change(Option<Shortcut>),
}

#[derive(Default)]
pub struct Recorder {
    recording: bool,
    held: Mods,
    error: Option<ShortcutError>,
    refused: Option<Shortcut>,
    finishing: Option<PhysicalKeyCode>,
}

impl Recorder {
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn error(&self) -> Option<&ShortcutError> {
        self.error.as_ref()
    }

    pub fn refused(&self) -> Option<&Shortcut> {
        self.refused.as_ref()
    }

    pub fn begin(&mut self) {
        self.recording = true;
        self.error = None;
        self.refused = None;
        self.finishing = None;
    }

    pub fn cancel(&mut self) {
        self.recording = false;
        self.held = Mods::default();
        self.error = None;
        self.refused = None;
    }

    pub fn keycaps(&self, bound: Option<&Shortcut>) -> Vec<String> {
        if let Some(refused) = self.refused.as_ref().filter(|_| self.recording) {
            refused.keycaps()
        } else if self.recording {
            modifier_keycaps(self.held)
        } else {
            bound.map(Shortcut::keycaps).unwrap_or_default()
        }
    }

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

    pub fn finish(&mut self, next: Option<Shortcut>, result: Result<(), ShortcutError>) {
        match result {
            Ok(()) => self.cancel(),
            Err(error) => {
                self.error = Some(error);
                self.refused = next;
            }
        }
    }

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
