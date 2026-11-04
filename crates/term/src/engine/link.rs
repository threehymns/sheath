//! The hyperlink a cell carries (OSC 8), which a Cmd/Ctrl+click opens. Ghostty stores the URI
//! with the cells themselves, so this reads it back the same way the selection reads text.

use ghostty_vt::terminal::{Point, PointCoordinate};

use super::Engine;

impl Engine {
    /// The cells a link covers on its row: the hovered cell's link, walked out while the URI
    /// stays the same. The cost is one row of hit tests at worst, and the pane only asks when
    /// the pointer moves to another cell, so a pointer crossing a link costs nothing.
    pub fn hyperlink_span(&self, at: (u16, u16)) -> Option<(u16, u16)> {
        let uri = self.hyperlink_at(at)?;
        let same = |col: u16| self.hyperlink_at((col, at.1)).as_deref() == Some(uri.as_str());
        let mut start = at.0;
        while start > 0 && same(start - 1) {
            start -= 1;
        }
        let mut end = at.0;
        while end + 1 < self.cols && same(end + 1) {
            end += 1;
        }
        Some((start, end))
    }

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
