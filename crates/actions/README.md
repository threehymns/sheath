# actions

The keymap kernel every app shares: what an action is, which chord resolves to it, and how a
shortcut is recorded. No files, no widgets, no app types.

## Scope

* **Commands are the app's enum.** An app defines its own `enum Action` and implements
  [`Action`] for it. The kernel is generic over that type, so `perform` stays an exhaustive
  `match` and the app keeps its names, labels and per-OS defaults.
* **No files.** Reading, writing, watching and atomic replacement are the host's job (see
  `actions-kdl` for one codec). This crate never touches `std::fs`.
* **No widgets.** It depends on `rune` only, for `Shortcut`, `KeyEvent` and `PhysicalInput`.
  Icons, palette rows and settings pages live in `actions-ui`.
* **No app vocabulary.** Nothing here knows a config file name, a folder, or what an app draws.

Depth lives at `Keybinds<A>`: defaults, conflict detection, physical-key matching, keycaps and
menu labels sit behind eleven methods. A second app reuses all of it by defining an enum.

When a chord may fire is the app's policy. The kernel matches chords; the host decides whether
a text field, the terminal pane or a modal should swallow the key first.

## How the rule is enforced

`tests/scope.rs` fails on a dependency beyond `rune`, on `std::fs`/`notify` in the sources, and
when the README loses its `## Scope` section. Cargo enforces the dependency direction: this crate
does not declare `actions-ui` or any app crate, so it cannot import them.

## Using it

```rust
use actions::{Action, Keybinds};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Command { Rename, Copy }

impl Action for Command {
    const ALL: &'static [Self] = &[Command::Rename, Command::Copy];
    fn from_id(id: &str) -> Option<Self> { /* "rename" -> Rename */ }
    fn id(self) -> &'static str { /* ... */ }
    fn label(self) -> &'static str { /* ... */ }
    fn default_chord(self) -> Option<&'static str> { Some("f2") }
}

let binds = Keybinds::<Command>::defaults();
if binds.matches(Command::Rename, &event) {
    run(Command::Rename);
}
```
