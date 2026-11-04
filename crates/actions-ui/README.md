# actions-ui

The surfaces every app with actions gets: the command palette, the keybind editor, and the card
rows a settings page is built from. They are generic over the app's `Action` enum and over the
host, so two apps share them without sharing state.

## Scope

* **It knows actions, settings and shortcuts.** Not files, not paths, not config formats.
  Reading and writing a keybinds file is `actions-kdl`'s job; the host calls it.
* **It owns no application state.** The palette's query and selection live in
  `actions_ui::Palette`, which the host stores; everything else comes through `PaletteHost` and
  `KeybindHost`. There is no `App` type here.
* **It draws, it does not decide.** `enabled`, `run`, `record`, `reset` and `saved_in` are host
  methods. The crate has no policy about what a command does or where a binding is saved.
* **Dependencies stay at `actions`, `rune` and `rune-kit`.** No `chrome`, no `scroll`, no theme.

The dialog shell, nav rail and corner notch are the host's: a settings dialog differs between
apps, while a row and a recorder field do not. The host wraps these pieces in its own frame.

## How the rule is enforced

`tests/scope.rs` fails on a dependency beyond `actions`/`rune`/`rune-kit`, on a source that
names `std::fs`, `notify`, `kdl`, `chrome`, `scroll` or `rune_tty`, and when the README loses
its `## Scope` section. The trait bounds are the second enforcement: without
`PaletteHost`/`KeybindHost`, a widget cannot reach into app state, so a leak shows up as a
compile error at the call site.

## Using it

```rust
impl actions_ui::PaletteHost<Action> for App { /* eight methods */ }
impl actions_ui::KeybindHost<Action> for App { /* seven methods */ }

let (scrim, slot) = actions_ui::palette::layers(self, k, Action::icon);
layers.push(scrim);
layers.push(slot);
```

Keys are stable (`command-<id>`, `record-<id>-field`, `reset-binding-<id>`, `settings-shortcuts`)`
so an app's tests keep addressing the same widgets.
