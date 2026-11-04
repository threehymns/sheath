//! The shell's side of the pane: the PTY, the shell on the other end, and the reader thread
//! that keeps the bytes coming. Nothing here knows what a terminal screen is.

use std::collections::VecDeque;
use std::ffi::OsString;
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
pub const QUEUE_LIMIT: usize = 2 * 1024 * 1024;

/// Bytes the reader takes off the pty per read.
const CHUNK: usize = 16 * 1024;

/// The shell and the two ends of its PTY, with the queue the reader thread fills.
pub struct Session {
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
pub struct Incoming {
    queue: Mutex<VecDeque<u8>>,
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
            queue: Mutex::new(VecDeque::new()),
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
        queue.extend(bytes);
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

    /// Takes up to `budget` bytes from the front, oldest first, into `out`, and wakes the reader
    /// if there is room again. The buffer is the caller's, so a burst that parses chunk after
    /// chunk does not allocate one per chunk.
    pub fn take_into(&self, budget: usize, out: &mut Vec<u8>) {
        out.clear();
        let Ok(mut queue) = self.queue.lock() else {
            return;
        };
        if queue.is_empty() {
            return;
        }
        // Draining the front leaves the rest where it is: a full queue is not copied per chunk,
        // so catching up on a burst costs the bytes parsed and not the backlog behind them.
        let take = budget.min(queue.len());
        out.extend(queue.drain(..take));
        drop(queue);
        self.spent(take);
        self.room.notify_all();
    }

    /// The same take as a fresh buffer, for a test that reads what came out.
    #[cfg(test)]
    pub fn take(&self, budget: usize) -> Vec<u8> {
        let mut out = Vec::new();
        self.take_into(budget, &mut out);
        out
    }

    pub fn len(&self) -> usize {
        self.queue.lock().map(|queue| queue.len()).unwrap_or(0)
    }

    /// How long the oldest unparsed byte has waited. A frame drawn now shows the screen as it was
    /// this long ago.
    pub fn lag(&self) -> Duration {
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

/// How a shell is started so it loads Ghostty's integration scripts: bash in POSIX mode with the
/// script in `ENV`, zsh through `ZDOTDIR`, and fish and elvish through `XDG_DATA_DIRS`. The
/// scripts are the ones libghostty ships, written to a cache directory by `resources_dir`.
struct Integration {
    /// Arguments the shell needs to pick the scripts up.
    args: Vec<&'static str>,
    /// Variables to set for it, in the order they are set.
    env: Vec<(&'static str, OsString)>,
}

/// The integration for `shell`, or `None` when it is a shell without a script. `lookup` reads the
/// environment the shell would otherwise inherit, so the values this replaces are handed back to
/// it and its startup files see what they expect.
fn integration(
    shell: &Path,
    resources: &Path,
    lookup: &dyn Fn(&str) -> Option<OsString>,
) -> Option<Integration> {
    let name = shell.file_name()?.to_str()?;
    let mut env = Vec::new();
    let args = match name {
        "bash" => {
            // In POSIX mode bash reads `ENV` instead of its startup files, and the script
            // recreates that sequence before handing control back. `GHOSTTY_BASH_INJECT` tells
            // it that it was injected rather than sourced by hand, so it restores POSIX mode's
            // off state and history file too.
            if let Some(existing) = lookup("ENV") {
                env.push(("GHOSTTY_BASH_ENV", existing));
            }
            env.push(("GHOSTTY_BASH_INJECT", OsString::from("1")));
            env.push((
                "ENV",
                resources
                    .join("shell-integration/bash/ghostty.bash")
                    .into_os_string(),
            ));
            vec!["--posix"]
        }
        "zsh" => {
            // zsh sources `$ZDOTDIR/.zshenv` first; the directory holds a .zshenv that restores
            // the user's own before loading the integration.
            if let Some(existing) = lookup("ZDOTDIR") {
                env.push(("GHOSTTY_ZSH_ZDOTDIR", existing));
            }
            env.push((
                "ZDOTDIR",
                resources.join("shell-integration/zsh").into_os_string(),
            ));
            Vec::new()
        }
        "fish" | "elvish" => {
            // Both read vendor directories from XDG_DATA_DIRS, and both remove this directory
            // from it again once the script has loaded.
            let mut dirs = resources.as_os_str().to_owned();
            match lookup("XDG_DATA_DIRS") {
                Some(existing) if !existing.is_empty() => {
                    dirs.push(":");
                    dirs.push(existing);
                }
                _ => dirs.push(":/usr/local/share:/usr/share"),
            }
            env.push(("XDG_DATA_DIRS", dirs));
            env.push((
                "GHOSTTY_SHELL_INTEGRATION_XDG_DIR",
                resources.as_os_str().to_owned(),
            ));
            Vec::new()
        }
        _ => return None,
    };
    Some(Integration { args, env })
}

/// Opens a PTY, starts the user's shell on it, and puts a thread on the reading end.
pub fn spawn(
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
    // Ghostty's integration scripts make the shell report its directory (OSC 7) and mark its
    // prompts (OSC 133) without any setup from the user, which is what the header's path and a
    // program's prompt-aware redraws rely on. `SHEATH_SHELL_INTEGRATION=none` opts out, for a
    // shell whose own startup files would not survive the POSIX-mode detour bash takes.
    let enabled = !matches!(
        std::env::var("SHEATH_SHELL_INTEGRATION").as_deref(),
        Ok("none" | "0" | "false")
    );
    if enabled
        && let Ok(resources) = ghostty_vt::shell_integration::resources_dir()
        && let Some(integration) =
            integration(Path::new(&shell), &resources, &|key| std::env::var_os(key))
    {
        for arg in &integration.args {
            command.arg(arg);
        }
        for (key, value) in integration.env {
            command.env(key, value);
        }
    }
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
pub fn close(incoming: &Incoming) {
    incoming.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The environment the shell would inherit: what the integration has to preserve.
    fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| OsString::from(*value))
        }
    }

    fn value(integration: &Integration, key: &str) -> Option<OsString> {
        integration
            .env
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.clone())
    }

    const RESOURCES: &str = "/usr/share/ghostty";

    /// bash is started in POSIX mode so it reads `ENV` instead of its startup files; the script
    /// then recreates that sequence itself. `GHOSTTY_BASH_INJECT` is what tells it it was
    /// injected, so it knows to leave POSIX mode and restore the user's history file.
    #[test]
    fn bash_loads_the_script_from_env_in_posix_mode() {
        let integration = integration(Path::new("/bin/bash"), Path::new(RESOURCES), &lookup(&[]))
            .expect("bash has an integration");
        assert_eq!(integration.args, vec!["--posix"]);
        assert_eq!(
            value(&integration, "GHOSTTY_BASH_INJECT").as_deref(),
            Some("1".as_ref())
        );
        assert_eq!(
            value(&integration, "ENV"),
            Some(OsString::from(
                "/usr/share/ghostty/shell-integration/bash/ghostty.bash"
            ))
        );
        assert_eq!(
            value(&integration, "GHOSTTY_BASH_ENV"),
            None,
            "there was no ENV of the user's to preserve"
        );
    }

    /// An ENV the user already had is handed back to the script, which restores it once it has
    /// finished, so the shell after startup sees what it would have seen without the pane.
    #[test]
    fn bash_hands_the_users_own_env_back_to_the_script() {
        let integration = integration(
            Path::new("/bin/bash"),
            Path::new(RESOURCES),
            &lookup(&[("ENV", "/home/me/.env")]),
        )
        .expect("bash has an integration");
        assert_eq!(
            value(&integration, "GHOSTTY_BASH_ENV"),
            Some(OsString::from("/home/me/.env"))
        );
    }

    /// zsh reads `$ZDOTDIR/.zshenv` first, so the pane points it at the integration directory and
    /// hands the user's own directory back for the script to restore.
    #[test]
    fn zsh_points_zdotdir_at_the_scripts() {
        let integration = integration(
            Path::new("/usr/bin/zsh"),
            Path::new(RESOURCES),
            &lookup(&[("ZDOTDIR", "/home/me/.config/zsh")]),
        )
        .expect("zsh has an integration");
        assert!(integration.args.is_empty(), "zsh needs no arguments");
        assert_eq!(
            value(&integration, "ZDOTDIR"),
            Some(OsString::from("/usr/share/ghostty/shell-integration/zsh"))
        );
        assert_eq!(
            value(&integration, "GHOSTTY_ZSH_ZDOTDIR"),
            Some(OsString::from("/home/me/.config/zsh"))
        );
    }

    /// fish and elvish read vendor directories from XDG_DATA_DIRS; the script takes the pane's
    /// directory back out of it once it has loaded, and the rest of the list is untouched.
    #[test]
    fn fish_prepends_the_vendor_directory_to_xdg_data_dirs() {
        let integration = integration(
            Path::new("/usr/bin/fish"),
            Path::new(RESOURCES),
            &lookup(&[("XDG_DATA_DIRS", "/home/me/.local/share")]),
        )
        .expect("fish has an integration");
        assert!(integration.args.is_empty());
        assert_eq!(
            value(&integration, "XDG_DATA_DIRS"),
            Some(OsString::from("/usr/share/ghostty:/home/me/.local/share"))
        );
        assert_eq!(
            value(&integration, "GHOSTTY_SHELL_INTEGRATION_XDG_DIR"),
            Some(OsString::from(RESOURCES)),
            "the script needs to know which entry to remove again"
        );
    }

    /// A shell without a script is started exactly as it would have been.
    #[test]
    fn a_shell_without_a_script_is_started_untouched() {
        for shell in ["/bin/sh", "/bin/dash", "/bin/ksh", "/usr/bin/nu"] {
            assert!(
                integration(Path::new(shell), Path::new(RESOURCES), &lookup(&[])).is_none(),
                "{shell} has no integration script"
            );
        }
    }

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
