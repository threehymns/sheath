//! The mouse as a program sees it: whether it asked for the gesture at all, the buttons and the
//! wheel encoded into the protocol it selected, and the release that names the button which
//! ended. The pane decides when a gesture belongs to the program; this encodes it.

use super::Terminal;

/// Which mouse gesture the pane is reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Drag,
    Release,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
    pub fn mouse_tracking(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.mouse_tracking();
        }
        false
    }

    pub fn mouse(&mut self, action: MouseAction, at: (u16, u16), shift: bool) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            let (action, button) = match action {
                MouseAction::Press => (
                    ghostty_vt::mouse::Action::Press,
                    Some(ghostty_vt::mouse::Button::Left),
                ),
                MouseAction::Drag => (
                    ghostty_vt::mouse::Action::Motion,
                    Some(ghostty_vt::mouse::Button::Left),
                ),
                MouseAction::Release => (
                    ghostty_vt::mouse::Action::Release,
                    Some(ghostty_vt::mouse::Button::Left),
                ),
            };
            let bytes = engine.mouse(action, button, at, shift);
            if !bytes.is_empty() {
                engine.write(&bytes);
                return true;
            }
        }
        let _ = (action, at, shift);
        false
    }

    /// One wheel notch for a program that captured the mouse: button 4 is up and 5 is down,
    /// which is how `less`, `vim` and `htop` scroll their own view. Says whether the program
    /// took it; the pane scrolls its own history when none did.
    pub fn mouse_wheel(&mut self, up: bool, at: (u16, u16), shift: bool) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            if !engine.mouse_tracking() {
                return false;
            }
            let button = if up {
                ghostty_vt::mouse::Button::Four
            } else {
                ghostty_vt::mouse::Button::Five
            };
            // A whole notch is a press in the mouse protocols: the encoder turns button 4 or 5
            // into the wheel code the program reads, and a release for it would read as a
            // second notch in some programs.
            let bytes = engine.mouse(ghostty_vt::mouse::Action::Press, Some(button), at, shift);
            if !bytes.is_empty() {
                engine.write(&bytes);
                return true;
            }
        }
        let _ = (up, at, shift);
        false
    }
}
