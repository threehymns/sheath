use super::{App, home_dir, root_dir};
use crate::drives::{DriveAction, Finished};

impl App {
    /// Opens a drive, mounting it first when it is not mounted yet. An unmounted drive starts
    /// its mount on a worker; the finished command opens the folder from `finish_drive`.
    pub fn open_drive(&mut self, id: &str) {
        match self.drives.mount(id) {
            Ok(Some(path)) => {
                self.error = None;
                self.open_place(path);
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }

    /// Unmounts a drive. A folder on the drive leaves it first, so the window never shows a
    /// path that has just gone away.
    pub fn unmount_drive(&mut self, id: &str) {
        match self.drives.unmount(id) {
            Ok(Some(mounted)) => {
                self.error = None;
                self.leave_drive(&mounted);
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }

    /// Lands a worker's answer: opens a newly mounted drive, or leaves a folder on one that has
    /// just been unmounted.
    pub(super) fn finish_drive(&mut self, finished: Finished) {
        if let Some(error) = finished.error {
            self.error = Some(error);
            return;
        }
        let Some(path) = finished.path else {
            return;
        };
        self.error = None;
        match finished.action {
            DriveAction::Mount => self.open_place(path),
            DriveAction::Unmount => self.leave_drive(&path),
        }
    }

    fn leave_drive(&mut self, mounted: &std::path::Path) {
        if self.cwd.starts_with(mounted) {
            self.navigate(home_dir().unwrap_or_else(root_dir));
        }
    }
}
