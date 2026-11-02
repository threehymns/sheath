//! A window that is only the terminal pane. See "Run" in README.md:
//!
//! ```sh
//! cargo run --bin terminal            # a shell in the folder you are in
//! cargo run --bin terminal -- ~/src   # ... or in another one
//! ```

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    sheath::launch_terminal();
}

/// The browser build has no shell, so it has no terminal window either.
#[cfg(target_arch = "wasm32")]
fn main() {}
