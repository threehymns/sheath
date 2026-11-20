//! A log line for frames that took far longer than a frame should.
//!
//! A freeze leaves nothing behind: the window stops painting, and when it comes back there is no
//! trace of where it went. The poll hook marks the moment a frame begins, the view marks the
//! moment it has built, and a frame that took longer than [`LONG`] writes a line to the app's
//! log. Everything a frame does on the main thread runs between those two marks — the poll hook,
//! the queued input, the press and key handlers and the view — so a handler that waited on a
//! sleeping disk is timed, not just the list read.

use std::cell::Cell;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A frame's own work this long or longer is written down. A 60 Hz frame takes 16 ms: this is
/// high enough that sorting a big folder does not fill the log, and low enough that anything a
/// person would call a freeze is caught.
pub const LONG: Duration = Duration::from_millis(250);

/// The log, in the config folder beside the KDL files, so a person who sees a freeze can hand
/// it over without having started the app from a terminal.
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
const LOG: &str = "sheath.log";

#[derive(Default)]
pub struct Stalls {
    /// When the frame that is building began, marked by the poll hook and cleared by the view.
    started: Cell<Option<Instant>>,
}

impl Stalls {
    /// Marks the start of a frame. The poll hook runs before the frame's input, its handlers and
    /// its view, so everything one frame does on the main thread is timed from here.
    pub fn begin(&self) {
        self.started.set(Some(Instant::now()));
    }

    /// Reports the frame that just built, if it took too long.
    ///
    /// A frame whose poll drew nothing can leave the mark standing until the next view, so the
    /// message says how long the app went without finishing a frame rather than claiming the
    /// work itself blocked.
    pub fn end(&self, folder: &Path) {
        let Some(started) = self.started.take() else {
            return;
        };
        let spent = started.elapsed();
        if spent < LONG {
            return;
        }
        write(&format!(
            "{} no frame finished for {:.1} s while showing {}",
            stamp(SystemTime::now()),
            spent.as_secs_f64(),
            folder.display()
        ));
    }
}

/// Appends one line to the log. A test keeps the lines in this thread so it can read them back
/// without touching the config folder.
fn write(line: &str) {
    #[cfg(target_arch = "wasm32")]
    let _ = line;
    #[cfg(test)]
    LOG_LINES.with_borrow_mut(|lines| lines.push(line.to_owned()));
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Debug builds already print Rune's own callback warnings here; a stall belongs with
        // them, and a release build started from a terminal gets the same line.
        eprintln!("sheath: {line}");
        #[cfg(not(test))]
        if let Some(dir) = crate::config::dir() {
            use std::io::Write;
            let _ = std::fs::create_dir_all(&dir);
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(LOG))
            {
                let _ = writeln!(file, "{line}");
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    static LOG_LINES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn take_log() -> Vec<String> {
    LOG_LINES.take()
}

/// The moment a stall is written down, in UTC: `2026-11-20 01:09:03Z`.
fn stamp(at: SystemTime) -> String {
    let seconds = at
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, time) = (seconds / 86_400, seconds % 86_400);
    // Days to a civil date, as Howard Hinnant's `civil_from_days` does it. A log line needs the
    // day and the clock, not a calendar crate.
    let shifted = days as i64 + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
        time / 3_600,
        time % 3_600 / 60,
        time % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamp_names_the_day_and_the_clock_in_utc() {
        assert_eq!(stamp(UNIX_EPOCH), "1970-01-01 00:00:00Z");
        assert_eq!(
            stamp(UNIX_EPOCH + Duration::from_secs(1_700_000_000)),
            "2023-11-14 22:13:20Z"
        );
        // A leap day, so the month and day maths is exercised past a February.
        assert_eq!(
            stamp(UNIX_EPOCH + Duration::from_secs(1_709_164_800)),
            "2024-02-29 00:00:00Z"
        );
    }

    #[test]
    fn a_slow_frame_is_written_down() {
        let _ = take_log();
        let stalls = Stalls::default();
        stalls.started.set(Some(Instant::now() - LONG));
        stalls.end(Path::new("/Volumes/Backup/Photos"));
        let lines = take_log();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("no frame finished for"), "{lines:?}");
        assert!(lines[0].contains("/Volumes/Backup/Photos"), "{lines:?}");
    }

    #[test]
    fn a_frame_that_kept_up_writes_nothing() {
        let _ = take_log();
        let stalls = Stalls::default();
        stalls.begin();
        stalls.end(Path::new("/home/reader"));
        assert!(take_log().is_empty(), "a frame inside the budget is quiet");
        // The view already took the mark, so a second report has nothing to say.
        stalls.end(Path::new("/home/reader"));
        assert!(take_log().is_empty(), "one frame is reported once");
    }
}
