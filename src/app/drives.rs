use super::{App, home_dir, root_dir};

impl App {
    /// Opens a drive, mounting it first when it is not mounted yet.
    pub fn open_drive(&mut self, id: &str) {
        match self.drives.mount(id) {
            Ok(path) => {
                self.error = None;
                self.open_place(path);
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// Unmounts a drive. A folder on the drive leaves it first, so the window never shows a
    /// path that has just gone away.
    pub fn unmount_drive(&mut self, id: &str) {
        match self.drives.unmount(id) {
            Ok(Some(mounted)) => {
                self.error = None;
                if self.cwd.starts_with(&mounted) {
                    self.navigate(home_dir().unwrap_or_else(root_dir));
                }
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
    }
}
