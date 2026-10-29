# Sheath

A file manager built with [Rune](https://github.com/anomalyco/rune) for its
views, layout, input, animation and rendering. It has a sidebar of places, a breadcrumb path
bar, a sortable file list, live search, an inspector with a text preview, and copy, move,
rename and trash operations.

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

## Controls

| Action | Control |
| --- | --- |
| Select rows | Click; Shift+click or Shift+arrow for a range; Cmd/Ctrl+click to toggle |
| Select with a box | Drag in the empty space below the rows; Shift adds, Cmd/Ctrl toggles |
| Open a folder or file | Double-click, or Return |
| Jump to a name | Type its first letters; repeat a letter to cycle matches |
| Back / forward | Cmd/Ctrl+[ and Cmd/Ctrl+] |
| Search this folder | The search field; Escape clears it |
| Sort or resize | Click a column header; drag a header edge |
| Rename / move / trash | F2, Shift+Cmd/Ctrl+M, Cmd/Ctrl+Backspace |
| Copy, cut, paste, duplicate | Cmd/Ctrl+C, X, V, D |
| Command bar | Cmd/Ctrl+P |
| Settings | Cmd/Ctrl+, |

Double-clicking a file opens it with the system opener (`open`, `xdg-open` or `start`). The
settings panel can change any shortcut.

## Configuration

Settings and keybinds are KDL files in one config folder:

- Linux: `$XDG_CONFIG_HOME/sheath` (`~/.config/sheath` by default)
- macOS: `~/Library/Application Support/Sheath`
- Windows: `%APPDATA%\Sheath`
- Any platform: `$SHEATH_CONFIG_DIR` overrides the others

`config.kdl` holds the view settings, all of which the settings panel can also change:

```kdl
settings {
    show-hidden #false
    inspector #true
    scroll-inertia #true
    overscroll #true
}
```

`scroll-inertia` lets the list coast after the wheel stops; `overscroll` lets it pull past the
first or last row and spring back.

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
command names are `command-bar`, `open-settings`, `back`, `forward`, `enclosing-folder`,
`refresh`, `new-folder`, `toggle-hidden`, `rename`, `move`, `trash`, `copy`, `cut`, `paste`
and `duplicate`. Recording a chord in Settings > Keybinds is easier, and it refuses a chord
another command already uses.

## Not there yet

No search inside subfolders, no filesystem watching, no image previews. Copy and paste use the
app's own clipboard rather than the system one.
