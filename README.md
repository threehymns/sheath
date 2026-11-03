# Sheath

A file manager built with [Rune](https://github.com/anomalyco/rune) for its
views, layout, input, animation and rendering. It has a reorderable sidebar of places, a
breadcrumb path bar, a sortable file list or tile grid, folder and entry rules in its config,
live search of the folder and everything below it, an inspector with text and picture previews, and copy,
move, rename and trash operations.

## Run

Requires a recent stable Rust toolchain (edition 2024, built with 1.98), a GPU adapter
(Metal, Vulkan or DX12), and Zig 0.17 on `PATH`. The terminal pane builds Ghostty's VT core from
a pinned source tarball at build time, and its build script looks for `zig` (or `$ZIG`). The web
build does not: it uses no terminal.

```sh
cargo run                 # start where you left off, or in your home folder the first time
cargo run -- ~/Downloads  # start somewhere else
cargo run -- README.md    # start beside a file, with it selected
```

The terminal pane is also a window of its own — the same engine, rows and scroll, with no file
manager around them:

```sh
cargo run --bin terminal           # a shell in the folder you are in
cargo run --bin terminal -- ~/src  # ... or in another one
```

Arguments after `--` go to the app; Rune parses its own flags first. The framework is pinned
to a `dev` commit in `Cargo.toml`, so the first build fetches Rune from git and takes a while.

## Web build

Rune's web target draws the same views in a browser tab through one WebGPU canvas. A tab has no
filesystem, so the web build browses an in-memory demo folder: opening, renaming, copying,
moving, creating and deleting work inside the tab, and a reload starts over.

The KDL files are real: `config.kdl`, `keybinds.kdl`, `views.kdl`, `session.kdl` and
`places.kdl` live in the browser's origin-private file system (OPFS), so settings, shortcuts,
remembered folders and sidebar places survive a reload of the page. They stay in that browser's
profile for that origin, not in the config folder the desktop build uses.

The folder button in the path bar opens the browser's folder picker and shares a real folder
with the tab through the File System Access API, with read-write permission. The folder appears
in the sidebar under `/granted` and can be browsed, searched, previewed, renamed, copied,
moved, deleted and written to like the demo tree, and pictures preview there: the browser
decodes the file (EXIF orientation included) and Rune draws the RGBA. An open folder stays
shared for the tab's lifetime; grant it again after a reload. Aliases and the system Trash
need the desktop build.

Opening files with the system, the terminal, the system Trash and folder watching are native
only; pictures in the demo tree are fetched and decoded by the browser instead of by the app.

Requires the wasm target, Python 3.11+ and the wasm-bindgen CLI matching `Cargo.lock`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
scripts/web
python3 -m http.server 8000 -d target/web   # or: bun scripts/serve.js
```

Open <http://localhost:8000> in a browser with WebGPU. `scripts/web` finds the Rune checkout
Cargo fetched and runs its packager, writing a static bundle to `target/web`; serve the whole
folder, `snippets/` included. Serve the folder, never `index.html` on its own: the page imports
`app.js` and fetches `app_bg.wasm` as modules, so a server rooted elsewhere (or one that rewrites
missing paths to HTML) answers with `text/html` and the browser refuses it as a module.

Release builds are optimized with `wasm-opt -Oz`, which takes `app_bg.wasm` from about 14.8 MB
to 7.2 MB (4.1 MB to 3.0 MB gzipped). If Binaryen is not on `PATH`, the script uses `npx` or
`bunx` to fetch it; `--no-opt` skips the pass, and `--dev` never runs it. Rune's `embedded-fonts`
default is off: it bundles Inter and IBM Plex Mono as fallbacks, but the app registers the theme's
faces itself, so those 1.3 MB would never be used.

First paint does not wait for the app: the page draws its own shell as soon as the HTML parses
and starts the wasm download during parsing, and wasm-bindgen compiles the module while it
streams. The module cannot be split into lazy chunks — a Rust `cdylib` with wasm-bindgen is one
module, and about 1.9 MB of it is static data (the theme's fonts and glyph tables) that the first
frame needs anyway. The web build cannot borrow fonts from the OS either: Rune shapes and
rasterises text itself, its font lookup is empty on `wasm32`, and only the app's registered bytes
exist in a tab. Cutting transfer size is the lever that works, which is why `bun
scripts/serve.js` gzips the bundle instead of sending 8.6 MB.

With Bun, the repository's own static server does the same:

```sh
bun scripts/serve.js            # http://localhost:3000
PORT=8080 bun scripts/serve.js
```

`--dev` builds faster and about three times as large, and `--watch` rebuilds and reloads the
page on a save:

```sh
scripts/web --dev --watch
```

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

## Terminal

The bottom pane is a real terminal: a shell on a PTY in the folder on screen, with Ghostty's VT
core parsing what the program writes and Rune drawing the cells. It keeps ~250,000 lines of
scrollback, reflows on resize, and follows the theme's foreground, background and cursor colours
while keeping Ghostty's ANSI palette. Desktop builds stream the grid as one texture;
`cargo run --features text-renderer` builds the earlier text-row renderer instead, and the web
build always uses it.

- **Collapsing, and closing.** The header's chevron (or the pane's own shortcut) collapses the
  pane; the shell keeps running and the same session comes back. The close button ends the
  shell, so opening the pane again starts a fresh one.
- **Resizing.** Drag the pane's top edge; the shell reflows as it moves. A drag stops at the
  minimum height; pushing a little further past it collapses the pane, and dragging the handle
  up from the bottom opens it again once the drag reaches the minimum. A click on the collapsed
  handle opens it at the height it had.
- **Scrolling.** The wheel and a trackpad scroll the history smoothly, pixel by pixel rather than
  a row at a time. Anything you type jumps back to the live tail.
- **Selecting.** Drag over the grid to select, double-click a word, triple-click a line, or
  Alt+drag a block of columns; **Ctrl+Shift+A** (Cmd+Shift+A on macOS) selects it all.
  **Ctrl+Shift+C** (Cmd+Shift+C on macOS) copies the selection, and **Ctrl+Shift+V** pastes,
  wrapping in bracketed paste when the program asked for it.
- **Searching its history.** **Ctrl+Shift+F** (Cmd+Shift+F on macOS) opens a field over the grid:
  every match is marked, Return and Shift+Return walk them, and Escape closes it.
- **Links.** Cmd+click (Ctrl+click off macOS) opens the link under the pointer when a program
  printed one with OSC 8. `http`/`https`, `mailto` and `file` links go to the system's launcher;
  any other scheme is left alone rather than handed to an arbitrary handler.
- **Programs that want the mouse** get the clicks, drags and wheel instead of a selection, so
  `htop`, `vim` and `less` behave as they do in Ghostty. Hold Shift to take the mouse back and
  select text.
- **What a program can ask it for.** Titles (OSC 0/2), the shell's directory (OSC 7, which the
  header falls back to), notifications for the desktop (OSC 9 and 777, as toasts), its own
  progress (OSC 9;4, drawn in the pane's header), its own background and colours (OSC 10/11),
  links over its text (OSC 8), the clipboard (OSC 52), focus in and out (mode 1004), which
  light or dark side of the theme it is drawing on, the `xterm-ghostty` terminfo entry, and
  synchronized output (mode 2026), so a screen a TUI redraws never shows its half-written
  frame. Underlines keep the style and colour they were given: single, doubled, curly, dotted
  or dashed, in SGR 58's colour when it set one.
- **Which keys reach the shell.** While the pane has focus it keeps everything, Ctrl-C included;
  only the command bar, Settings and the pane's own shortcut reach the app. Clicking the file list
  takes the keyboard back.

The pane's height is remembered in `session.kdl`, so it opens at the size it was left. The web
build has no shell and says so.

## Controls

| Action | Control |
| --- | --- |
| Select rows | Click; Shift+click or Shift+arrow for a range; Cmd/Ctrl+click to toggle |
| Select all, none or the rest | Cmd/Ctrl+A, Shift+Cmd/Ctrl+A, or the command bar |
| Select with a box | Drag in the empty space below the rows; Shift adds, Cmd/Ctrl toggles |
| Open a folder or file | Double-click, or Return |
| Jump to a name | Type its first letters; repeat a letter to cycle matches |
| Back / forward | Cmd/Ctrl+[ and Cmd/Ctrl+] |
| Go to a folder by path | Cmd/Ctrl+L |
| Search this folder and below | The search field, or / to focus it; Escape clears it |
| Sort or resize | Click a column header, or a sort button above the tiles; drag a header edge |
| Keep or drop a column, or sort folders first | Right-click a column header |
| List or grid | The layout button in the toolbar, or the command bar |
| Inspector on or off | The inspector button in the toolbar, or the command bar |
| Rename / move / trash | F2, Shift+Cmd/Ctrl+M, Cmd/Ctrl+Backspace |
| Delete for good | Shift+Delete (Shift+Cmd+Backspace on macOS) |
| New folder | Shift+Cmd/Ctrl+N, the toolbar button, or an empty folder's button |
| New file | Cmd/Ctrl+Alt+N |
| Copy, cut, paste, duplicate | Cmd/Ctrl+C, X, V, D |
| Make an alias | The row menu, or the command bar: `Make Alias` |
| Copy path | Shift+Cmd/Ctrl+C |
| Copy name | The row menu, or the command bar: `Copy Name` |
| Command bar | Cmd/Ctrl+P |
| Show or hide the terminal | Ctrl+Shift+T (Cmd+Shift+T on macOS), the row menu, or the command bar: `Open Terminal Here` |
| Copy the terminal selection | Ctrl+Shift+C (Cmd+Shift+C on macOS) |
| Paste into the terminal | Ctrl+Shift+V (Cmd+Shift+V on macOS) |
| Open a link in the terminal | Cmd/Ctrl+click |
| Reveal in the system file manager | The row menu, or the command bar: `Reveal in File Manager` |
| Reorder places | Drag a place in the sidebar |
| Add to the sidebar | Drag a folder or file from the list between sidebar places, or the folder menu's `Add This Folder to the Sidebar` |
| Place menu | Right-click a place in the sidebar |
| The folder's own menu | Right-click the empty space below the rows |
| Settings | Cmd/Ctrl+, |
| Rules | The command bar: `Show All Rules`, `Save This Layout as a Rule`, `View Rules for This Folder` / `Entry` |

Renaming several chosen rows at once names them in the order they read: the name you type, then a
number, then the extension each one keeps. Double-clicking a file opens it with the system opener
(`open`, `xdg-open` or `start`). Paste
lands in the folder on screen, or inside it when exactly one folder is selected. The settings
panel can change any shortcut.

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
    folders-first #true
    confirm-trash #true
    confirm-delete #true
    show-kind #true
    show-size #true
    show-modified #true
    scroll-inertia #true
    overscroll #true
}
```

`grid` shows the folder as tiles instead of a table of rows, with the same sort controls in a
bar of the header's height, and is also the toolbar's layout button. `folders-first` keeps folders
above files in either layout, `confirm-trash` asks before trashing, and `confirm-delete` asks
before deleting something for good. `show-kind`, `show-size` and `show-modified` keep or drop
those columns in the list and their sort buttons above the tiles. `scroll-inertia` lets every
scroll area coast after the wheel stops; `overscroll` lets it pull past its first or last row and
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

A `folder-rule` sets `layout` (`list`, `grid`), `sort` (`name`, `kind`, `size`, `modified`), `direction`
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
`enclosing-folder`, `go-to-folder`,
`refresh`, `new-folder`, `new-file`, `toggle-hidden`, `rename`, `move`, `trash`, `copy`,
`copy-path`, `copy-name`, `cut`, `paste`, `delete`,
`duplicate`, `select-all`, `deselect-all`, `invert-selection`, `open-terminal`, `reveal`,
`add-place`, `make-alias`, `toggle-layout`, `toggle-inspector`,
`toggle-folders-first`,
`view-folder-rules`, `view-entry-rules`, `save-layout-rule`, `show-entry`,
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

Beside it, `session.kdl` holds the folder the window was last in, so a launch that names no path
reopens there; a folder or file on the command line still wins. It is machine-owned and safe to
delete too.

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
`pictures`, `root` and `temp` follow the machine (`path` overrides where one opens),
`folder:<path>` is a folder of your own, and `file:<path>` is one of your files, which opens
its folder with the file selected. `name` overrides the label; a missing one is the folder's or
file's own name. Dragging a folder or file from the list onto a place moves it inside that
folder; dropping it between places inserts it there in the section, without moving anything
and with a preview of the landing spot. `volume:<id>` is left for drives a later release
detects, and a place whose folder is not there — an unmounted drive, a deleted folder — keeps
its row in the file and comes back with the folder. Right-clicking a place opens its menu: open
it, show it in its folder, edit the name it shows, rename the folder or file on disk, or remove
it. The sidebar's name belongs to the sidebar alone; renaming on disk moves both the item and
the place that points at it. A removed place keeps its row in the
file with `hidden #true`, and dragging its folder back restores that same row. Built-in places the file leaves out are added at the end of their
section, and top-level nodes the app does not know are written back untouched. Like
`views.kdl`, the file is machine-owned and safe to delete: the sidebar goes back to its
built-in places.

## Not there yet

No per-entry icons or badges. Copy and paste use the app's own clipboard rather than the system
one. The terminal draws text, colours, underline styles and search matches, but not Kitty
graphics or images, and has no IME pre-editing.
