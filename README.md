# Sheath

A file manager PoC built entirely with [Rune](https://github.com/anomalyco/rune)'s own view,
layout, input, animation and rendering layers. It is a normal desktop app: a sidebar of places,
a breadcrumb path bar, a virtualised sortable file list with box and modifier selection, live
search, hidden-file visibility, an inspector with file metadata and a text preview, copy/cut/
paste and duplicate, drag and drop onto folders and sidebar places, a right-click menu for
renaming, moving and trashing rows, a command bar, and a settings panel with its own sidebar
(General, Keybinds, About).

The app uses Rune's `SplitView` shell (`Sidebar`, `Toolbar`, panes), `TextInput`,
`Button`/`IconButton`, `Dialog`, `MenuItem`/`context_menu`, `Sidebar`/`Switch` for the settings
panel, `ShortcutRecorder` and `Shortcut`, OpenCode's Rune theme and fonts, the icon atlas, and
the same headless tooling the framework's examples use. The file list itself is app-drawn on
Rune's `virtual_rows`, so the app owns the rows, the selection and the drag box. Keybinds are
saved as KDL with the `kdl` crate.

## Run

Requires a recent stable Rust toolchain (edition 2024; verified with 1.98) and a desktop with
a GPU adapter (Metal, Vulkan or DX12).

```sh
cargo run                 # start in your home folder
cargo run -- ~/Downloads  # start somewhere else
cargo run -- README.md    # start beside a file, with it selected
```

Arguments after `--` go to the app; Rune's own flags are parsed first. To render a headless
snapshot (no window, no display needed):

```sh
cargo run -- --snap shot.png --size 1180x760 .
```

## Controls

| Action | Control |
| --- | --- |
| Select a row | Click, or Up/Down, Page Up/Page Down, Home/End |
| Select a range of rows | Shift-click, or Shift+Up/Down, Home/End |
| Add or remove a row | Cmd/Ctrl-click |
| Select every row | Cmd/Ctrl+A |
| Select with a box | Drag in the empty space below the rows |
| Add a box to the selection | Shift-drag, or Ctrl/Cmd+Shift-drag |
| Toggle rows with a box | Ctrl/Cmd-drag |
| Narrow a selection to one row | Escape, or click a row in it |
| Jump to a name | Type its first letters; repeat one letter to cycle matches |
| Open a folder / file | Double-click, or Return |
| Enclosing folder | Backspace, or Cmd/Ctrl+Up |
| Back / forward | Toolbar arrows, or Cmd/Ctrl+[ and Cmd/Ctrl+] |
| Refresh the folder | Toolbar refresh, or Cmd/Ctrl+R |
| New folder | Toolbar folder button, or Shift+Cmd/Ctrl+N |
| Show hidden files | Toolbar eye button, or Shift+Cmd/Ctrl+. |
| Search this folder | Click the search field; Escape clears it |
| Sort | Click a column header; click again to reverse |
| Resize a column | Drag the divider on a header's trailing edge |
| Resize a pane | Drag the edge of the sidebar or inspector |
| Rename / move / trash | Right-click a row, or the keyboard shortcuts below |
| Move rows into a folder | Drag the selection onto a folder row or a sidebar place |
| Copy / cut / paste / duplicate | Cmd/Ctrl+C, X, V, D |
| Commands | Cmd/Ctrl+P (or Cmd/Ctrl+Shift+P) for the command bar |
| Settings | Toolbar gear button, or Cmd/Ctrl+, |

Shortcuts are defaults: the Keybinds category of the settings panel records a new chord per
command, refuses a chord another command already uses, and Restore Defaults puts them back. The
defaults are:

| Command | Shortcut |
| --- | --- |
| Back / forward | Cmd/Ctrl+[ and Cmd/Ctrl+] |
| Enclosing folder | Cmd/Ctrl+Up (or Backspace alone) |
| Refresh | Cmd/Ctrl+R |
| New folder | Shift+Cmd/Ctrl+N |
| Show hidden files | Shift+Cmd/Ctrl+. |
| Rename | F2 |
| Move to folder | Shift+Cmd/Ctrl+M |
| Move to trash | Cmd/Ctrl+Backspace (or the Delete key) |
| Copy / cut / paste | Cmd/Ctrl+C, Cmd/Ctrl+X, Cmd/Ctrl+V |
| Duplicate | Cmd/Ctrl+D |
| Command bar | Ctrl+P (Shift included) |
| Settings | Cmd/Ctrl+, |

Double-clicking a file hands it to the system opener (`open`, `xdg-open`, or `start`). Folders
navigate in place, and back/forward keep their history while the search field is cleared.

## What the PoC covers

- Reading real folders with `std::fs`, following symlinks for display while marking them.
- Folders first, then natural name order by default; size and modified-date sorting from the
  column headers, with the sort arrow and draggable column widths.
- Virtualised rows, so a very large folder only builds the rows on screen.
- Live case-insensitive filtering by name, plus hidden-file visibility.
- Selection drives an inspector with kind, size, dates, Unix permissions, location, and a
  text preview for files up to 24 KB. Several selected rows become a summary with their count
  and total size, and Move and Trash act on all of them while Rename stays single-row.
- Multi-row selection: Shift-click and Shift+arrow for a range, Cmd/Ctrl-click to toggle a
  row, Cmd/Ctrl+Shift for a range added to what is selected, Cmd/Ctrl+A for everything, Escape
  to narrow back to one. The path bar counts a selection ("3 of 5 selected") and a right-click
  inside one keeps it.
- A rubber-band box: dragging in the list's empty space selects the rows it sweeps; Shift adds
  them, Cmd/Ctrl toggles them, and Cmd/Ctrl+Shift does both. Option-drag (Alt on Linux) starts
  the box on the rows themselves, so a list that fills the window can be swept too; held
  against the list's top or bottom edge the box scrolls it and takes the rows that pass under.
- Drag and drop: dragging a row (or a whole selection) onto a folder row or a sidebar place
  moves it there, with the target row highlighted and a badge following the pointer. Holding a
  drag against the list's top or bottom edge scrolls it under the pointer, faster the further
  past the edge it is pushed, and the highlighted target follows the rows that pass. A
  collision is refused before anything moves and reported in the banner.
- Copy, cut, paste and duplicate. A copy takes a free name ("notes copy.md", then "notes copy
  2.md"), a cut moves and is pasted once, folders are copied with their contents, and what a
  paste creates is selected. Cut rows wait dimmed until the paste.
- Typing a name jumps to the first row that starts with it, and repeating one letter cycles
  through the matches; the Delete key asks before trashing.
- Creating uniquely named folders and selecting the new one.
- Renaming in place, moving into a folder typed as a path (`~` opens your home folder), and
  moving to the system Trash, each behind a sheet. A sheet keeps itself open with the reason
  when the filesystem refuses (an existing name, a folder moved into itself, a destination that
  isn't a folder). Rename and move refuse to overwrite.
- A right-click row menu that selects the row (or keeps a selection it lands in) and opens at
  the pointer with the three file operations, closing on Escape, an outside press or a pick.
- A command bar (Cmd/Ctrl+P) listing every command the keybinds registry holds, filtered by
  name, run with Return or a click, with commands that need a selection dimmed.
- A settings panel that covers the window with its own sidebar: General toggles hidden files,
  the inspector, scroll inertia and overscroll, Keybinds holds a recorder per command, About
  shows the version and the keybinds file.
- Remappable shortcuts, recorded from the keyboard in the panel and matched by key position, so
  a recorded chord runs the same command after a restart as it did before.
- Errors such as permission-denied appear in a banner rather than blanking the window.

It deliberately does not search subfolders, watch the filesystem for changes, preview images,
or use the system clipboard for files (the clipboard is the app's own while it runs). Those are
obvious next steps rather than hidden behaviour.

## Code map

| File | What lives there |
| --- | --- |
| `src/app.rs` | Browser state, the selection and drag box, folder reads, sorting/filtering, navigation history, previews, the clipboard, the rename/move/trash sheets, command bar state, row menu state, keyboard shortcuts |
| `src/ui.rs` | The Rune view: shell, toolbar, sidebar, breadcrumb bar, the file list with its header and selection box, inspector, row menu, sheets and the settings panel |
| `src/format.rs` | Human sizes, dates, kinds and permissions |
| `src/icons.rs` | The extra SVG glyphs the browser draws |
| `src/keybinds.rs` | The commands shortcuts can run, their default chords, matching a key event, and the KDL keybinds file |
| `src/settings.rs` | The saved view settings and their own `config.kdl` |
| `src/tests.rs` | Model tests and headless `Runtime` tests that click, type and assert layouts |

## Settings and keybinds

The panel opens from the toolbar gear (or Cmd/Ctrl+,), with **General**, **Keybinds** and
**About** in its sidebar; Escape or Done returns to the browser. Settings and keybinds each
save to a file of their own, so editing one never rewrites the other. The view settings (hidden
files, the inspector, and the two scrolling behaviours) live in `config.kdl` beside this file:

```kdl
settings {
    show-hidden #false
    inspector #true
    scroll-inertia #true
    overscroll #true
}
```

Scroll inertia carries the wheel's own motion: the rows follow a trackpad's deltas 1:1 while
the fingers move, then coast at the speed they had and decay over about a second, while a notch
of a notched wheel leaves a shorter glide. A hard flick carries as far as the hand threw it, and
where the platform reports a gesture's phases they count as the fingers: down stops the rows
where they are, even if they never lifted, and resting them for a moment clears the speed, so
lifting after a pause starts nothing. Overscroll lets a wheel at the first or last row pull the
rows up to 72 points out of place and springs them back. Both start on, so a fresh app scrolls
the way the platform does; either can be turned off in General. The wheel handling lives in
`src/app.rs` (`list_wheel` and the per-frame `poll`), because Rune's scroll viewport applies
wheel deltas 1:1 and clamps them to its ends. It reads the viewport's position from `El::on_scroll_offset` rather than from
the rows' rects, which Rune reports a frame late and only past a movement threshold: starting a
delta from a stale position made a fast trackpad scroll jump back.

Keybinds live in `keybinds.kdl`:

- Linux: `$XDG_CONFIG_HOME/sheath/keybinds.kdl` (`~/.config/sheath/keybinds.kdl` by default)
- macOS: `~/Library/Application Support/Sheath/keybinds.kdl`
- Windows: `%APPDATA%\Sheath\keybinds.kdl`
- Any platform: `SHEATH_CONFIG_DIR/keybinds.kdl` when that variable is set

```kdl
// Sheath keyboard shortcuts. Only keybinds live in this file; other settings get their
// own. An empty chord clears the shortcut; leave the file out to get the defaults.
keybinds {
    open-settings ctrl+comma
    back ctrl+bracketleft
    rename f5
    move ""
}
```

Chords are written the way Rune's shortcuts read them (`ctrl`, `alt`, `shift`, `super`, then a
key name), so the file can be edited by hand. Anything else the app might save one day belongs
in a separate file, not this one.

## Framework version

Dependencies are pinned to Rune's `dev` commit
[`b85cd7d4c408c30f91c299069624958345c224e4`](https://github.com/anomalyco/rune/commit/b85cd7d4c408c30f91c299069624958345c224e4),
as Rune's app guidance recommends. Update the `rev` in `Cargo.toml` to move to a newer one.

## Tests

```sh
cargo test
```

The 93 tests cover folder ordering, hidden files, search, sorting, history, new-folder naming,
rename/move/trash including their refusal cases, text and folder previews, copy/cut/paste and
duplicate (files and folders, unique names, cuts pasting once), keybinds (matching, recorded
changes, conflicts, the KDL round trip and a broken file), the saved settings files, the
scrolling options and their momentum and bounce, plus
headless Rune runs that select and open rows, walk rows with the keyboard, extend, toggle and
Ctrl+Shift-click a selection, draw a box with each modifier combination (empty space and
Option-dragged over the rows), drag rows onto a folder row and onto a sidebar place (and refuse a collision), scroll the list they are dragged
against, resize a column, jump to a name
by typing, right-click rows (in every column) to open the row menu, pick its items, run the
command bar by key and by click, walk the settings sidebar and its categories, toggle the
General switches, record a chord there, and build both the single-file preview and the
selection summary.
