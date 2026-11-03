//! A window that is only the terminal pane. See "Run" in README.md:
//!
//! ```sh
//! cargo run --example terminal            # a shell in the folder you are in
//! cargo run --example terminal -- ~/src   # ... or in another one
//! ```
//!
//! It is an example rather than a second binary because Rune's web packager builds a package's
//! one binary (or one cdylib) for wasm, and refuses a package with two: this window has no
//! browser build, and keeping it out of the binary targets keeps `scripts/web` unambiguous.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    sheath::launch_terminal();
}

/// The browser build has no shell, so it has no terminal window either.
#[cfg(target_arch = "wasm32")]
fn main() {}
