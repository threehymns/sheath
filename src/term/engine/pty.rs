//! The shell's side of the pane: the PTY, the shell on the other end, and the reader thread
//! that keeps the bytes coming. Nothing here knows what a terminal screen is.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use rune::Waker;

/// Bytes the reader will queue before it stops reading. With the queue full the pty stays full,
/// the program blocks in its next write, and its own frame rate becomes the rate the screen can
/// show. Without a limit a program that draws as fast as it can runs away from the screen: the
/// queue grows without end, every frame parses more than the one before it, and the pane falls
/// further behind the longer it runs.
pub(super) const QUEUE_LIMIT: usize = 2 * 1024 * 1024;

/// Bytes the reader takes off the pty per read.
const CHUNK: usize = 16 * 1024;

/// The shell and the two ends of its PTY, with the queue the reader thread fills.
pub(super) struct Session {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    /// Bytes the reader thread has taken off the PTY and not yet parsed.
    pub incoming: Arc<Incoming>,
    /// Set while a frame is pending, so a burst of output asks for one frame and not one each.
    pub pending: Arc<AtomicBool>,
}

/// The screen's inbox: bytes the shell wrote and the app has not parsed yet, and the room the
/// reader waits for once the queue is full.
pub(super) struct Incoming {
    queue: Mutex<Vec<u8>>,
    /// Signalled when the app takes bytes, so the reader can read again.
    room: Condvar,
    /// Each queued chunk and when it arrived, oldest first: the front's age is how far the
    /// screen is behind the shell.
    arrivals: Mutex<VecDeque<(usize, Instant)>>,
    /// Set when the pane ends, so a reader waiting for room stops instead of waiting forever.
    closed: AtomicBool,
}

impl Incoming {
    fn new() -> Incoming {
        Incoming {
            queue: Mutex::new(Vec::new()),
            room: Condvar::new(),
            arrivals: Mutex::new(VecDeque::new()),
            closed: AtomicBool::new(false),
        }
    }

    /// Adds bytes, and remembers when they arrived so the lag can be read back.
    fn push(&self, bytes: &[u8]) {
        let Ok(mut queue) = self.queue.lock() else {
            return;
        };
        queue.extend_from_slice(bytes);
        drop(queue);
        if let Ok(mut arrivals) = self.arrivals.lock() {
            arrivals.push_back((bytes.len(), Instant::now()));
        }
    }

    /// Drops the arrival times of the bytes just taken, so the front is always the oldest byte
    /// still waiting rather than the first byte that ever filled the queue.
    fn spent(&self, len: usize) {
        let Ok(mut arrivals) = self.arrivals.lock() else {
            return;
        };
        let mut left = len;
        while left > 0 {
            let Some((count, _)) = arrivals.front_mut() else {
                break;
            };
            let take = (*count).min(left);
            *count -= take;
            left -= take;
            if *count == 0 {
                arrivals.pop_front();
            }
        }
    }

    /// Takes up to `budget` bytes from the front, oldest first, and wakes the reader if there is
    /// room again.
    pub(super) fn take(&self, budget: usize) -> Vec<u8> {
        let Ok(mut queue) = self.queue.lock() else {
            return Vec::new();
        };
        if queue.is_empty() {
            return Vec::new();
        }
        let rest = if queue.len() > budget {
            queue.split_off(budget)
        } else {
            Vec::new()
        };
        let bytes = std::mem::replace(&mut *queue, rest);
        drop(queue);
        self.spent(bytes.len());
        self.room.notify_all();
        bytes
    }

    pub(super) fn len(&self) -> usize {
        self.queue.lock().map(|queue| queue.len()).unwrap_or(0)
    }

    /// How long the oldest unparsed byte has waited. A frame drawn now shows the screen as it was
    /// this long ago.
    pub(super) fn lag(&self) -> Duration {
        self.arrivals
            .lock()
            .ok()
            .and_then(|arrivals| arrivals.front().map(|(_, at)| at.elapsed()))
            .unwrap_or(Duration::ZERO)
    }

    /// Ends the pane: a reader parked on a full queue leaves instead of waiting for room that
    /// will never come.
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.room.notify_all();
    }
}

/// Opens a PTY, starts the user's shell on it, and puts a thread on the reading end.
pub(super) fn spawn(
    dir: &Path,
    cols: u16,
    rows: u16,
    cell: (f32, f32),
    wake: Waker,
) -> Result<Session, String> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: (cols as f32 * cell.0) as u16,
            pixel_height: (rows as f32 * cell.1) as u16,
        })
        .map_err(|err| format!("Couldn't open a terminal: {err}"))?;
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    let mut command = CommandBuilder::new(&shell);
    command.cwd(dir);
    // The screen is Ghostty's VT core, so the shell is told it is on that terminal. Its
    // terminfo entry is what makes programs use synchronized output, the kitty keyboard
    // protocol and the rest of what the core answers; without the entry on disk, the plain
    // xterm the core also speaks is the honest fallback.
    match ghostty_vt::terminfo::dir() {
        Ok(terminfo) => {
            command.env("TERM", ghostty_vt::terminfo::TERM);
            command.env("TERMINFO", terminfo);
        }
        Err(_) => {
            command.env("TERM", "xterm-256color");
        }
    }
    command.env("COLORTERM", "truecolor");
    // Programs read these to pick a terminal's own behaviours, the way they do in Ghostty.
    command.env("TERM_PROGRAM", "ghostty");
    command.env(
        "TERM_PROGRAM_VERSION",
        ghostty_vt::build_info::pinned_version(),
    );
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|err| format!("Couldn't start {shell}: {err}"))?;
    drop(pair.slave);
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| format!("Couldn't read the terminal: {err}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|err| format!("Couldn't write the terminal: {err}"))?;

    let incoming = Arc::new(Incoming::new());
    let pending = Arc::new(AtomicBool::new(false));
    read_forever(reader, Arc::clone(&incoming), Arc::clone(&pending), wake);
    Ok(Session {
        master: pair.master,
        writer,
        child,
        incoming,
        pending,
    })
}

/// Copies the shell's output into the queue on its own thread. It never parses, and it stops
/// reading while the queue is full: a program that writes faster than the app draws waits in its
/// next write, as it would in any terminal, instead of filling memory with a backlog the screen
/// may never catch up with.
fn read_forever(
    mut reader: Box<dyn Read + Send>,
    incoming: Arc<Incoming>,
    pending: Arc<AtomicBool>,
    wake: Waker,
) {
    std::thread::spawn(move || {
        let mut chunk = [0u8; CHUNK];
        loop {
            if let Ok(mut queue) = incoming.queue.lock() {
                while queue.len() >= QUEUE_LIMIT && !incoming.closed.load(Ordering::Acquire) {
                    match incoming
                        .room
                        .wait_timeout(queue, Duration::from_millis(100))
                    {
                        Ok((next, _)) => queue = next,
                        Err(_) => return,
                    }
                }
                if incoming.closed.load(Ordering::Acquire) {
                    break;
                }
            } else {
                break;
            }
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => {
                    if !pending.swap(true, Ordering::AcqRel) {
                        wake();
                    }
                    break;
                }
                Ok(read) => {
                    incoming.push(&chunk[..read]);
                    // Coalesce a burst into one frame: a wake already queued will drain
                    // everything this reader added.
                    if !pending.swap(true, Ordering::AcqRel) {
                        wake();
                    }
                }
            }
        }
    });
}

/// The pane ends, so a reader waiting for room is let go.
pub(super) fn close(incoming: &Incoming) {
    incoming.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_queue_gives_one_budget_at_a_time_and_keeps_the_rest() {
        let incoming = Incoming::new();
        assert_eq!(incoming.len(), 0);
        assert!(incoming.take(64).is_empty());
        incoming.push(b"0123456789");
        assert_eq!(incoming.len(), 10);
        assert_eq!(incoming.take(4), b"0123");
        assert_eq!(incoming.len(), 6);
        assert!(
            incoming.lag() > Duration::ZERO,
            "the rest of the chunk is still waiting"
        );
        assert_eq!(incoming.take(64), b"456789");
        assert_eq!(incoming.len(), 0);
        assert_eq!(incoming.lag(), Duration::ZERO, "nothing is waiting now");
        incoming.push(b"x");
        assert!(incoming.lag() > Duration::ZERO, "a queued byte is late");
        incoming.close();
    }
}
