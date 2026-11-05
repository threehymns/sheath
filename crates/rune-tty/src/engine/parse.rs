//! The parse loop: the bytes the reader took off the pty, the events libghostty raises while it
//! reads them, and the replies the program is owed. Everything here runs on the app thread,
//! between the pane's frames.

use std::path::PathBuf;
use std::time::Instant;

use ghostty_vt::terminal::{ColorScheme, Event, ProgressState};

use super::{Engine, PARSE_CHUNK};
use crate::{Progress, ProgressKind, Theme};

impl Engine {
    /// Parses what the reader brought, in chunks that respect the frame's time budget, and
    /// collects the replies the program asked for. Says whether anything the pane draws changed.
    pub fn parse(&mut self, replies: &mut Vec<u8>, until: Instant) -> bool {
        let mut changed = false;
        let mut bytes = std::mem::take(&mut self.buffers.parse);
        loop {
            self.incoming.take_into(PARSE_CHUNK, &mut bytes);
            if bytes.is_empty() {
                break;
            }
            self.vt.vt_write(&bytes);
            self.parsed += bytes.len() as u64;
            changed = true;
            changed |= self.absorb_all(replies);
            // Past the budget the rest waits for the next frame: a backlog is drawn over many
            // frames at the display's rate, instead of one frame that takes as long as the
            // backlog is deep while the reader keeps adding to it.
            if Instant::now() >= until {
                break;
            }
        }
        self.buffers.parse = bytes;
        changed
    }
}

impl Engine {
    /// Applies every event libghostty queued for the bytes just written, and says whether the
    /// pane draws differently for any of them. The event buffer is reused: a burst raises
    /// events per chunk, and a fresh vector per chunk would be an allocation per chunk.
    pub fn absorb_all(&mut self, replies: &mut Vec<u8>) -> bool {
        let mut raised = std::mem::take(&mut self.events);
        raised.extend(self.vt.events());
        let mut changed = false;
        for event in raised.drain(..) {
            changed |= self.absorb(event, replies);
        }
        self.events = raised;
        changed
    }

    /// Applies one side effect libghostty raised, and collects the reply the program is owed.
    /// Says whether the pane draws differently for it. The parse loop and the test feed share
    /// this, so a test sees the same clipboard, title and progress a shell's bytes produce.
    pub fn absorb(&mut self, event: Event, replies: &mut Vec<u8>) -> bool {
        match event {
            // Device attributes, cursor reports and mode answers, on their way back.
            Event::PtyWrite(data) => replies.extend(data),
            Event::Bell => self.bell.set(true),
            Event::TitleChanged(title) => {
                self.title = title;
                // The pane's header is named by the title, so a program that renames its window
                // renames it too.
                return true;
            }
            // OSC 52 and the other clipboard protocols, on their way to the app's clipboard.
            Event::ClipboardWrite(write) => {
                for content in write.contents {
                    if content.mime.starts_with("text/")
                        && let Ok(text) = String::from_utf8(content.data)
                    {
                        self.clipboard.push(trim_trailing(text));
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
            // Mode 2026: the program is assembling a frame and has asked the screen not to
            // show the parts of it written so far.
            Event::RenderHold(held) => {
                self.hold = held.then(Instant::now);
                return true;
            }
        }
        false
    }
}

/// Which side of a theme a background sits on, for the color-scheme report a program queries.
pub fn scheme(theme: Theme) -> ColorScheme {
    let lum = |c: u8| {
        let value = f32::from(c) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * lum(theme.bg.r) + 0.7152 * lum(theme.bg.g) + 0.0722 * lum(theme.bg.b);
    if luminance > 0.5 {
        ColorScheme::Light
    } else {
        ColorScheme::Dark
    }
}

/// The directory an OSC 7 report names: a `file://` URI whose path may be percent-encoded, a
/// `kitty-shell-cwd://` one (the scheme Ghostty's integration scripts report with), or a bare
/// path from one of the protocols that reports one. Anything that does not decode to a path is
/// dropped, so the header keeps showing the last directory that did.
pub fn directory(report: &str) -> Option<PathBuf> {
    let path = match report
        .strip_prefix("file://")
        .or_else(|| report.strip_prefix("kitty-shell-cwd://"))
    {
        // `scheme://host/path`: the host is the local machine when it is empty, and the first
        // slash after it starts the path either way.
        Some(rest) => &rest[rest.find('/')?..],
        None => report,
    };
    let path = decode(path);
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// A copy with its trailing whitespace trimmed, which is what Ghostty hands the clipboard by
/// default (`clipboard-trim-trailing-spaces`): a `yank` or a selection almost always ends in a
/// newline, and keeping it pastes an empty line wherever the copy lands.
pub fn trim_trailing(mut text: String) -> String {
    let len = text.trim_end_matches([' ', '\t', '\r', '\n']).len();
    text.truncate(len);
    text
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
pub fn progress_of(state: ProgressState, percent: Option<u8>) -> Option<Progress> {
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
