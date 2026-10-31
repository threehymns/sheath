//! What the app tells the user about a config file it cannot read: a toast that says which file
//! stopped it and where, and a report its Copy button hands over, whole, for an editor or an
//! assistant. The report is written to be read away from this app: file, place, the line itself,
//! and the shape the file was being read against.

use std::time::{Duration, Instant};

/// How long a toast stays. Long enough to read the place and press Copy, short enough that it is
/// gone before it becomes part of the furniture.
pub const SECONDS: u64 = 20;

/// One file the app could not read, while it is worth saying so.
pub struct Notice {
    /// The file's name, which is what the toast is keyed by and what keeps two notices apart.
    pub file: &'static str,
    /// What the toast calls it: `config.kdl does not parse`.
    pub title: String,
    /// The one line under the title.
    pub detail: String,
    /// Everything Copy puts on the clipboard.
    pub report: String,
    /// When it arrived, so it can leave on its own.
    pub shown: Instant,
    /// Whether Copy has been pressed, so the toast can say so instead of offering it twice.
    pub copied: bool,
}

impl Notice {
    pub fn new(file: &'static str, title: String, detail: String, report: String) -> Notice {
        Notice {
            file,
            title,
            detail,
            report,
            shown: Instant::now(),
            copied: false,
        }
    }

    /// Whether the toast has been up long enough to go.
    pub fn expired(&self) -> bool {
        self.shown.elapsed() >= Duration::from_secs(SECONDS)
    }

    /// The key the toast is drawn with.
    pub fn key(&self) -> String {
        format!("notice-{}", self.file)
    }

    /// What Copy puts on the clipboard: the report, with the file it came from at the top, so a
    /// paste is self-contained even when it starts mid-conversation.
    pub fn clipboard(&self) -> String {
        self.report.clone()
    }
}
