# scroll

A scroll area's physics for Rune apps: wheel inertia, overscroll past the ends, the drag that
pushes a list, and the small registry a view builds its areas from.

Sheath's list, grid, inspector, settings page and command bar all share it; any Rune app with a
scrollable surface can.

## Scope

* **Behaviour, not layout.** It owns offsets, velocity, overshoot and the spring back. It does
  not know what the content is, what a row is, or why an area exists.
* **No app vocabulary and no domain dependencies.** No files, no paths, no settings, no actions.
  The host answers two questions: what the area's limits are, and whether a resize matters to it.
* **`rune` only.** Not `rune-kit`, not any other workspace crate.
* **The host keeps its own per-frame loop.** `Areas::begin_frame` and `step_key` return the
  steps; `apply` hands them to `Tx`. The crate never owns the window.

## How the rule is enforced

`tests/scope.rs` fails on a dependency beyond `rune`, on a source that names another workspace
crate (or `std::fs`/`notify`), and when the README loses its `## Scope` section. The physics
tests in `tests/scroll.rs` came with the code and drive `Scroll` directly, with no app and no
window.

## Using it

```rust
impl scroll::Host for App {
    fn areas(&self) -> &scroll::Areas { &self.areas }
    fn areas_mut(&mut self) -> &mut scroll::Areas { &mut self.areas }
    fn scroll_limits(&self, _key: &'static str) -> scroll::Limits { /* from settings */ }
    fn area_resized(&mut self, _key: &'static str, _rect: [f32; 4]) -> bool { false }
}

let list = scroll::area(self, "files-table", view, content);
```
