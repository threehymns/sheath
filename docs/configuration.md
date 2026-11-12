# Configuration

This reference lives beside the README; see [the README](../README.md) for the app itself.

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
    zebra-stripes #true
    folders-first #true
    confirm-trash #true
    confirm-delete #true
    show-kind #true
    show-size #true
    show-modified #true
    show-thumbnails #true
    system-icons #false
    scroll-inertia #true
    overscroll #true
}
```

`grid` shows the folder as tiles instead of a table of rows, with the same sort controls in a
bar of the header's height, and is also the toolbar's layout button. `zebra-stripes` alternates
row backgrounds in the list layout. `show-thumbnails` draws a
picture for each image, falling back to the file icon. `system-icons` draws folders and files
with the desktop's own icons.
`folders-first` keeps folders
above files, `confirm-trash` asks before trashing, and `confirm-delete` asks
before deleting something for good. `show-kind`, `show-size` and `show-modified` keep or drop
those columns. `scroll-inertia` lets every
scroll area coast after the wheel stops; `overscroll` lets it pull past its first or last row and
spring back.

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
and with a preview of the landing spot. `volume:<id>` names a drive the sidebar detected, and a
place whose folder is not there — an unmounted drive, a deleted folder — keeps its row in the
file and comes back with the folder. Right-clicking a place opens its menu: open
it, show it in its folder, edit the name it shows, rename the folder or file on disk, or remove
it. The sidebar's name belongs to the sidebar alone; renaming on disk moves both the item and
the place that points at it. A removed place keeps its row in the
file with `hidden #true`, and dragging its folder back restores that same row. Built-in places the file leaves out are added at the end of their
section, and top-level nodes the app does not know are written back untouched. Like
`views.kdl`, the file is machine-owned and safe to delete: the sidebar goes back to its
built-in places.

The sidebar also shows a **Drives** section of its own, after the places. It is never written to
`places.kdl`: it lists the external drives the machine reports, mounted or not. Clicking an
unmounted drive mounts it and opens it, and a mounted drive carries an unmount button at the
left of its row. The list refreshes every few seconds, so a drive plugged in later appears
without a restart.
