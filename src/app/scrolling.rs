use std::cell::Cell;
use std::rc::Rc;

use rune::{ScrollPos, Tx};
use scroll::{Areas, Host, Limits};

use super::*;

// The file table and the grid share this viewport, and the drag maths works from its offset.
pub const FILES: &str = "files-table";

pub fn poll(s: &mut App, t: f64, tx: &mut Tx) -> bool {
    s.poll(t, tx)
}

impl scroll::Host for App {
    fn areas(&self) -> &Areas {
        &self.areas
    }

    fn areas_mut(&mut self) -> &mut Areas {
        &mut self.areas
    }

    fn scroll_limits(&self, key: &'static str) -> Limits {
        Limits {
            max: self.told(key).get().max[1].max(0.0),
            inertia: self.config.scroll_inertia,
            bounce: self.config.overscroll,
        }
    }

    /// Records a viewport's rect from its layout, and says whether anything the view reads
    /// moved: only the list's own rect sizes the marquee and the grid's columns.
    fn record_area(&mut self, key: &'static str, rect: [f32; 4]) -> bool {
        let sized = key == FILES && self.rows_view != rect;
        if key == FILES {
            self.rows_view = rect;
        }
        self.areas.set_region(key, rect);
        sized
    }
}

impl App {
    // A view tells every scroll area it builds where it is, so drags know what they are over.
    pub fn begin_view(&self) {
        self.areas.begin_view();
    }

    // The offset cell a viewport writes its position into; the view keeps it across frames.
    pub fn told(&self, key: &'static str) -> Rc<Cell<ScrollPos>> {
        self.areas.offset_cell(key)
    }

    fn told_offset_of(&self, key: &'static str) -> f32 {
        self.told(key).get().offset[1].max(0.0)
    }

    pub(super) fn told_offset(&self) -> f32 {
        self.told_offset_of(FILES)
    }

    pub fn overshoot(&self, key: &str) -> f32 {
        self.areas.overshoot(key)
    }

    // The list's offset, which the drag and marquee maths work from.
    pub(super) fn list_scroll(&self) -> f32 {
        self.areas.current(FILES)
    }

    pub(super) fn stop_scrolling(&mut self) {
        self.areas.stop();
    }

    pub(super) fn stop_coasting(&mut self) {
        self.areas.stop_coasting();
    }

    pub(super) fn stop_bouncing(&mut self) {
        self.areas.stop_bouncing();
    }

    pub fn poll(&mut self, t: f64, tx: &mut Tx) -> bool {
        #[cfg(test)]
        self.settle_scan();
        let mut redraw = self.land_scan();
        redraw |= self.land_find();
        redraw |= self.land_preview();
        redraw |= self.land_thumbs();
        self.request_thumbs();
        redraw |= self.land_folder_size();
        if let Some(id) = self.pending_reveal.take() {
            tx.scroll_into_view_in(FILES, id);
        }
        if self.store.poll() {
            self.apply_config();
            redraw = true;
        }
        if self.watch.poll() {
            self.reload();
            redraw = true;
        }
        // A folder listing, a file read, or the folder picker finished in the browser.
        if crate::vfs::take_dirty() {
            self.reload();
            redraw = true;
        }
        if let Some(path) = crate::vfs::take_granted() {
            self.accept_granted_folder(path);
            redraw = true;
        }
        if let Some(error) = crate::vfs::take_error() {
            self.error = Some(error);
            redraw = true;
        }
        if !self.notices.is_empty() {
            let before = self.notices.len();
            self.notices.retain(|notice| !notice.expired());
            redraw |= self.notices.len() != before;
        }
        if let Some(edit) = self.rule_edit.as_mut() {
            let dirty = edit.dirty();
            if edit.shown_dirty != dirty {
                edit.shown_dirty = dirty;
                redraw = true;
            }
        }
        if let Some((key, wait)) = self.rule_reveal.clone() {
            if wait == 0 {
                self.rule_reveal = None;
                if self.settings == Some(SettingsTab::Rules) {
                    tx.scroll_key_to(key, 8.0);
                }
            } else {
                self.rule_reveal = Some((key, wait - 1));
                redraw = true;
            }
        }
        // A file drag or a marquee in flight tracks the rows it sits over, and its badge, its
        // insert line and the box all read the list's position, so a frame that slid the rows
        // must rebuild. Everything else paints the moved viewport itself and skips the view.
        let tracking =
            self.file_drag.is_some() || self.marquee.as_ref().is_some_and(|m| m.dragging);
        for key in self.areas.known() {
            let dt = self.areas.begin_frame(key, t);
            let limits = self.scroll_limits(key);
            // A drag pushes the area under the pointer; the other areas stand still.
            let push = self
                .drag_push(dt)
                .filter(|(target, _)| *target == key)
                .map_or(0.0, |(_, points)| points);
            let out = self.areas.step_key(key, limits, push, tx);
            redraw |= out.redraw || (tracking && out.moved);
        }
        self.follow_drag();
        redraw |= self.poll_terminal(tx);
        redraw
    }
}
