//! The shell's side of the pane: the PTY, the shell on the other end of it, and the reader thread
//! that keeps the bytes coming. Nothing here knows what a terminal screen is.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use rune::Waker;

/// The shell and the two ends of its PTY, with the queue the reader thread fills.
pub(super) struct Session {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    /// Bytes the reader thread has taken off the PTY and not yet parsed.
    pub incoming: Arc<Mutex<Vec<u8>>>,
    /// Set while a frame is pending, so a burst of output asks for one frame and not one each.
    pub pending: Arc<AtomicBool>,
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
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
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

    let incoming = Arc::new(Mutex::new(Vec::new()));
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

/// Copies the shell's output into the queue on its own thread: it never parses, so a program that
/// writes faster than the app draws cannot hold the app thread, and one wake covers a burst.
fn read_forever(
    mut reader: Box<dyn Read + Send>,
    incoming: Arc<Mutex<Vec<u8>>>,
    pending: Arc<AtomicBool>,
    wake: Waker,
) {
    std::thread::spawn(move || {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => {
                    if !pending.swap(true, Ordering::AcqRel) {
                        wake();
                    }
                    break;
                }
                Ok(read) => {
                    let Ok(mut queue) = incoming.lock() else {
                        break;
                    };
                    queue.extend_from_slice(&chunk[..read]);
                    drop(queue);
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
