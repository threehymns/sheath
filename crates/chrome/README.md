# chrome

Dialog chrome for Rune apps. One thing so far: a bite out of a panel's corner for a control that
sits there, drawn because Rune only rounds corners outward.

## Scope

**`chrome` contains only what Rune cannot draw.**

If it can be built with `col`/`row`/`radius`/`fill`/`border`/`shadow`, it does not belong here.
That keeps the crate small enough that every rule below is checkable.

* It knows no application vocabulary: no actions, settings, keybinds, palettes, tabs or `App`.
* It depends on `rune` and nothing else. Not `rune-kit`, not any other crate in this workspace.
* Numbers that depend on a shape here stay here (`CornerControl::backdrop` and `::place`), so the
  cut and the control it holds cannot drift apart.

The dialog shell, scrim, elevation and layout live in the app or in `actions-ui`. This crate does
not grow into a design system. If it ever needs `Kit` or a tab, the code belongs elsewhere.

## How the rule is enforced

`tests/scope.rs` fails when the manifest gains a dependency beyond `rune`, when a source names
another workspace crate, or when this README loses its `## Scope` section. The dependency rule
is also enforced by Cargo: a crate that is not declared cannot be imported.

## Using it

```rust
use chrome::{Corner, CornerControl};

let cut = CornerControl {
    panel: [0.0, 0.0, 880.0, 600.0],
    corner: Corner::TopRight,
    radius: 16.0,
    inset: 8.0,
    size: (76.0, 26.0),
    gap: 8.0,
};
dialog
    .child(cut.backdrop(k.c.bg_deep, k.c.border_weak))
    .child(cut.place(done_button));
```

Paint the backdrop over the corner, then the control. The backdrop redraws the panel's border
around the bite; `place` puts the control in it.
