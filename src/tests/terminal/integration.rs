//! The shell integration the pane starts a shell with, checked against a real shell: the first
//! prompt reports the directory on its own (OSC 7), which is what the header falls back to.

#![cfg(not(target_arch = "wasm32"))]

use std::path::Path;
use std::time::{Duration, Instant};

use crate::term::engine::Engine;

/// A shell with an integration script is started with it, and its first prompt reports the
/// directory the pane started it in. A shell without one is started unchanged, so this only
/// checks the shells the pane can integrate.
#[test]
fn the_first_prompt_reports_the_directory_it_started_in() {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let name = Path::new(&shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    if !matches!(name.as_str(), "bash" | "zsh" | "fish") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        80,
        24,
        (8.0, 16.0),
        1.0,
        crate::term::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        engine.drain();
        if engine.pwd() == Some(dir.path()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "{name} reported no directory (OSC 7); it reported {:?}",
        engine.pwd().map(Path::to_path_buf)
    );
}
