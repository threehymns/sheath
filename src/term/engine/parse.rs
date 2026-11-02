//! The parse loop: the bytes the reader took off the pty, the events libghostty raises while it
//! reads them, and the replies the program is owed. Everything here runs on the app thread,
//! between the pane's frames.

use std::path::PathBuf;
use std::time::Instant;

use ghostty_vt::terminal::{Event, ProgressState};

use super::{Engine, PARSE_CHUNK};
use crate::term::{Progress, ProgressKind};

impl Engine {
    /// Parses what the reader brought, in chunks that respect the frame's time budget, and
    /// collects the replies the program asked for. Says whether anything the pane draws changed.
    pub(super) fn parse(&mut self, replies: &mut Vec<u8>, until: Instant) -> bool {
        let mut changed = false;
        loop {
            let bytes = self.incoming.take(PARSE_CHUNK);
            if bytes.is_empty() {
                break;
            }
            self.vt.vt_write(&bytes);
            self.parsed += bytes.len() as u64;
            changed = true;
            for event in self.vt.events() {
                match event {
                    // Device attributes, cursor reports and mode answers, on their way back.
                    Event::PtyWrite(data) => replies.extend(data),
                    Event::Bell => self.bell.set(true),
                    Event::TitleChanged(title) => {
                        self.title = title;
                    }
                    // OSC 52 and the other clipboard protocols, on their way to the app's
                    // clipboard.
                    Event::ClipboardWrite(write) => {
                        for content in write.contents {
                            if content.mime.starts_with("text/")
                                && let Ok(text) = String::from_utf8(content.data)
                            {
                                self.clipboard.push(text);
                            }
                        }
                    }
                    // OSC 9;4, the progress a program reports for its own work.
                    Event::ProgressReport { state, progress } => {
                        self.progress = progress_of(state, progress);
                    }
                    // OSC 9 and OSC 777, a notification the program addressed to the desktop.
                    Event::DesktopNotification { title, body } => {
                        self.notes.push((title, body));
                    }
                    // OSC 7 (and the OSC 9 / OSC 1337 forms): the shell's own directory.
                    Event::PwdChanged(report) => {
                        if let Some(dir) = directory(&report) {
                            self.pwd = Some(dir);
                        }
                    }
                    // Mode 2026: the program is assembling a frame and has asked the screen not
                    // to show the parts of it written so far.
                    Event::RenderHold(held) => {
                        self.hold = held.then(Instant::now);
                        changed = true;
                    }
                }
            }
            // Past the budget the rest waits for the next frame: a backlog is drawn over many
            // frames at the display's rate, instead of one frame that takes as long as the
            // backlog is deep while the reader keeps adding to it.
            if Instant::now() >= until {
                break;
            }
        }
        changed
    }
}

/// The directory an OSC 7 report names: a `file://` URI whose path may be percent-encoded, or a
/// bare path from one of the protocols that reports one. Anything that does not decode to a
/// path is dropped, so the header keeps showing the last directory that did.
pub(crate) fn directory(report: &str) -> Option<PathBuf> {
    let path = match report.strip_prefix("file://") {
        // `file://host/path`: the host is the local machine when it is empty, and the first
        // slash after it starts the path either way.
        Some(rest) => &rest[rest.find('/')?..],
        None => report,
    };
    let path = decode(path);
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// The percent-decoded form of a URI path, which is how a directory with a space in it arrives.
fn decode(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && at + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[at + 1..at + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            at += 3;
            continue;
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| path.to_owned())
}

/// OSC 9;4's state, in the pane's own terms: `Remove` means there is nothing left to draw, and
/// every other state is one the header has a shape for.
pub(crate) fn progress_of(state: ProgressState, percent: Option<u8>) -> Option<Progress> {
    let kind = match state {
        ProgressState::Remove => return None,
        ProgressState::Set => ProgressKind::Set,
        ProgressState::Indeterminate => ProgressKind::Indeterminate,
        ProgressState::Pause => ProgressKind::Pause,
        ProgressState::Error => ProgressKind::Error,
    };
    Some(Progress {
        kind,
        percent: percent.map(|value| value.min(100)),
    })
}
