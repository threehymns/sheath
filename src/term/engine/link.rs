//! The hyperlink a cell carries (OSC 8), which a Cmd/Ctrl+click opens. Ghostty stores the URI
//! with the cells themselves, so this reads it back the same way the selection reads text.

use ghostty_vt::terminal::{Point, PointCoordinate};

use super::Engine;

impl Engine {
    /// The hyperlink on the cell at a viewport point, if the program attached one with OSC 8.
    pub fn hyperlink_at(&self, at: (u16, u16)) -> Option<String> {
        let point = Point::Viewport(PointCoordinate {
            x: at.0,
            y: u32::from(at.1),
        });
        let cell = self.vt.grid_ref(point).ok()?;
        // A URI can be longer than the first guess, and the binding says how much it needs.
        let mut size = 256;
        loop {
            let mut buf = vec![0u8; size];
            match cell.hyperlink_uri(&mut buf) {
                Ok(0) => return None,
                Ok(len) => return String::from_utf8(buf[..len].to_vec()).ok(),
                Err(ghostty_vt::Error::OutOfSpace { required }) => {
                    if required <= size {
                        return None;
                    }
                    size = required;
                }
                Err(_) => return None,
            }
        }
    }
}
