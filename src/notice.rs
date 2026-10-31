use std::time::{Duration, Instant};

// Seconds a toast stays up, enough to read it and press Copy.
pub const SECONDS: u64 = 20;

pub struct Notice {
    pub file: &'static str,
    pub title: String,
    pub detail: String,
    pub report: String,
    pub shown: Instant,
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

    pub fn expired(&self) -> bool {
        self.shown.elapsed() >= Duration::from_secs(SECONDS)
    }

    pub fn key(&self) -> String {
        format!("notice-{}", self.file)
    }

    pub fn clipboard(&self) -> String {
        self.report.clone()
    }
}
