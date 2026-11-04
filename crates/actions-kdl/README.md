# actions-kdl

The KDL codec for `actions`: a `keybinds { <id> "<chord>" }` block and a
`settings { <name> #true }` block. This is sheath's file format; another app can write a JSON or
TOML codec against the same kernel and never touch this crate.

## Scope

* **Format only.** Parsing, writing one key and removing one key. No paths, no files, no
  watching, no atomic writes: the host owns those.
* **No app vocabulary.** It knows `Action` and `Setting` through their traits, not an app's
  enums, labels, icons or UI keys.
* **It does not decide what a binding means.** Defaults, conflicts and matching are `actions`.
  `read` applies the file's entries over `Keybinds::defaults()`; anything else is the kernel's.

Editing preserves the rest of the document: comments, indentation and unrelated nodes survive
because the codec rewrites one child in place.

## How the rule is enforced

`tests/scope.rs` fails when the manifest gains a dependency beyond `actions` and `kdl`, when a
source names another workspace crate, `std::fs` or `notify`, or when the README loses its
`## Scope` section.

## Using it

```rust
let keybinds = actions_kdl::keybinds::read::<Command>(store.doc("keybinds.kdl"));
let text = {
    let mut doc = store.doc("keybinds.kdl").cloned().unwrap_or_default();
    actions_kdl::keybinds::set(&mut doc, &keybinds, Command::Rename);
    doc.to_string()
};
```
