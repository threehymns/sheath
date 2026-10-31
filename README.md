# Sheath

A file manager built with [Rune](https://github.com/anomalyco/rune) for its
views, layout, input, animation and rendering. It has a reorderable sidebar of places, a
breadcrumb path bar, a sortable file list or tile grid, folder and entry rules in its config,
live search, an inspector with a text preview, and copy, move, rename and trash operations.

## Run

Requires a recent stable Rust toolchain (edition 2024, built with 1.98) and a GPU adapter
(Metal, Vulkan or DX12).

```sh
cargo run                 # start in your home folder
cargo run -- ~/Downloads  # start somewhere else
cargo run -- README.md    # start beside a file, with it selected
```

Arguments after `--` go to the app; Rune parses its own flags first. The framework is pinned
to a `dev` commit in `Cargo.toml`, so the first build fetches Rune from git and takes a while.

## Live editing

Rune's opt-in `hot` feature patches function bodies into the running window. The feature is
declared in `Cargo.toml`; normal `cargo run` builds still leave it off. `scripts/dev`
finds the Rune checkout Cargo already fetched for the pinned revision and runs its launcher
from there. It needs Python 3.13+ and Dioxus CLI 0.7.10 at `~/.cargo/bin/dx` (`RUNE_DX`
overrides the path):

```sh
cargo install dioxus-cli --version 0.7.10
scripts/dev --patch 'src/**/*.rs'
```

The first build is a full development build and takes a while. Compiler artifacts land in the
Rune checkout's `target/live-edit-build`, not Sheath's `target/`. Save a function-body edit in a
patched file and the existing window rebuilds its view; app state, selection, scroll offsets
and spring tracks stay put. A patched view that panics falls back to the startup view. Edits to
types, signatures, imports, constants, impl methods, startup code, dependencies or assets print
**Restart needed**; `scripts/dev` rebuilds and relaunches on its own, without migrating
state. `--patch` can repeat, so narrow it to the files you are editing.

This is a state-preserving loop, not a subsecond one: at this Rune commit the framework's own
README reports about 8 s median save-to-GPU for its Settings example. Physical iOS devices are
unsupported by this backend.

## Controls

| Action | Control |
| --- | --- |
| Select rows | Click; Shift+click or Shift+arrow for a range; Cmd/Ctrl+click to toggle |
| Select with a box | Drag in the empty space below the rows; Shift adds, Cmd/Ctrl toggles |
| Open a folder or file | Double-click, or Return |
| Jump to a name | Type its first letters; repeat a letter to cycle matches |
| Back / forward | Cmd/Ctrl+[ and Cmd/Ctrl+] |
| Search this folder | The search field, or / to focus it; Escape clears it |
| Sort or resize | Click a column header, or a sort button above the tiles; drag a header edge |
| List or grid | The layout button in the toolbar |
| Rename / move / trash | F2, Shift+Cmd/Ctrl+M, Cmd/Ctrl+Backspace |
| Copy, cut, paste, duplicate | Cmd/Ctrl+C, X, V, D |
| Command bar | Cmd/Ctrl+P |
| Reorder places | Drag a place in the sidebar |
| Settings | Cmd/Ctrl+, |
| Rules | The command bar: `Show All Rules`, `Save This Layout as a Rule`, `View Rules for This Folder` / `Entry` |

Double-clicking a file opens it with the system opener (`open`, `xdg-open` or `start`). The
settings panel can change any shortcut.

## Configuration

The settings, rules and keybinds are KDL files in one config folder:

- Linux: `$XDG_CONFIG_HOME/sheath` (`~/.config/sheath` by default)
- macOS: `~/Library/Application Support/Sheath`
- Windows: `%APPDATA%\Sheath`
- Any platform: `$SHEATH_CONFIG_DIR` overrides the others

`config.kdl` holds the browser settings, all of which the settings panel can also change:

```kdl
settings {
    show-hidden #false
    inspector #true
    grid #false
    scroll-inertia #true
    overscroll #true
}
```

`grid` shows the folder as tiles instead of a table of rows, with the same sort controls in a
bar of the header's height, and is also the toolbar's layout button. `scroll-inertia` lets the
list coast after the wheel stops; `overscroll` lets it pull past the first or last row and
spring back. `grid` is the fallback for a folder no rule and no remembered visit speaks for.

`config.kdl` also holds folder and entry rules, as top-level nodes beside `settings`, in file
order:

```kdl
// How a folder presents its own listing.
folder-rule "projects" {
    match path="~/Projects/**"
    layout grid
    sort modified
    direction descending
}

// A crowded folder switches itself.
folder-rule "downloads" {
    match path="~/Downloads" min-items=20
    layout list
}

// One entry's presence, wherever it is listed.
entry-rule "config" {
    match path="~/.config"
    hidden #false
}
```

A `folder-rule` sets `layout` (`list`, `grid`), `sort` (`name`, `size`, `modified`), `direction`
(`ascending`, `descending`) and `show-hidden`; an `entry-rule` sets `hidden`. Any property takes
`inherit` to undo what an earlier rule said.

`match` and `exclude` take globs: `path` for a folder or an entry, `name`, `kind` (`file`,
`folder`) and `extension` for entries, and `min-items`/`max-items` for folders. Several `match`
nodes OR together, the keys inside one AND, and any matching `exclude` vetoes the rule; a rule
with no `match` matches everything. Matching is case-sensitive, `~` expands to home, and both
`/` and `\` separate segments, so a file of rules works on every platform.

Every matching rule applies in file order, later rules winning property by property. Rules
outrank what a folder remembers, which outranks `settings`; a change made from the toolbar in
a ruled folder lasts until you leave. `config.kdl` is watched, so an edit in a text editor
reaches the open window. A file that does not parse leaves the last good rules in place, shows
the error in the Rules tab, and is never written back.

Settings > Rules has a section per kind, each listing its rules in file order: Add Rule opens a
form on a new rule — started from the folder on screen and its layout, or from the entry selected
in the browser — and Save writes it after the last rule of its kind, so the file keeps one block
per kind. Drag a rule to reorder it within its section, or delete it. Edit opens that rule as a form holding everything the
file can say: its name, each `match` and `exclude` with every key, and each property,
`inherit` included. `Save This Layout as a Rule`, `Always Show This Entry`
and `Always Hide This Entry` write a rule for what is on screen, and `Show All Rules` and the
two `View Rules for …` commands find the rule behind it, saying so when nothing matches.

`keybinds.kdl` holds the shortcuts. Delete the file, or press Restore Defaults in Settings, to
get the defaults back:

```kdl
keybinds {
    open-settings ctrl+comma
    back ctrl+bracketleft
    rename f5
    move ""
}
```

An empty chord clears the shortcut. Chords are `ctrl`, `alt`, `shift` and `super`, then a key
name, written the way Rune's shortcuts read them, so the file can be edited by hand. The
command names are `command-bar`, `focus-search`, `open-settings`, `back`, `forward`,
`enclosing-folder`,
`refresh`, `new-folder`, `toggle-hidden`, `rename`, `move`, `trash`, `copy`, `cut`, `paste`,
`duplicate`, `view-folder-rules`, `view-entry-rules`, `save-layout-rule`, `show-entry`,
`hide-entry` and `pick-rule`. The rule commands start with no chord. Recording a chord in
Settings > Keybinds is easier, and it refuses a chord another command already uses.
`keybinds.kdl` is watched, so a new chord works without a restart.

Each folder also remembers the layout and sort it was left in, in a file of its own outside the
config folder:

- Linux: `$XDG_STATE_HOME/sheath/views.kdl` (`~/.local/state/sheath` by default)
- macOS: `~/Library/Application Support/Sheath/State/views.kdl`
- Windows: `%LOCALAPPDATA%\Sheath\views.kdl`
- Any platform: `$SHEATH_STATE_DIR` overrides the folder

`views.kdl` is machine-owned and safe to delete: it forgets the remembered folders and nothing
else, and a damaged file is ignored.

The sidebar's places are kept in `places.kdl` beside it. The file is written when a place is
dragged, and its order is the order the sidebar shows:

```kdl
favourites {
    place "home"
    place "downloads"
    place "folder:/home/me/Projects" name="Projects" path="/home/me/Projects"
}

locations {
    place "root"
    place "temp"
    place "volume:1A2B-3C4D" name="Backup" path="/run/media/me/Backup"
}
```

A place is its `id`, never its position: `home`, `desktop`, `documents`, `downloads`,
`pictures`, `root` and `temp` follow the machine (`path` overrides where one opens), and
`folder:<path>` is a folder of your own. `name` overrides the label; a missing one is the
folder's own name. `volume:<id>` is left for drives a later release detects, and a place whose
folder is not there — an unmounted drive, a deleted folder — keeps its row in the file and
comes back with the folder. Built-in places the file leaves out are added at the end of their
section, and top-level nodes the app does not know are written back untouched. Like
`views.kdl`, the file is machine-owned and safe to delete: the sidebar goes back to its
built-in places.

## Not there yet

No search inside subfolders, no image previews, no per-entry icons or badges. Copy and paste
use the app's own clipboard rather than the system one.
