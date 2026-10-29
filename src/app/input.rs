//! Window-level keys: the command bar's, the shortcuts the keybinds name, and the file
//! list's own.

use crate::app::App;
use crate::keybinds::Action;
use rune::{Key, KeyEvent, Tx};

/// Window-level shortcuts and arrow navigation while no control owns the keyboard.
pub fn on_key(s: &mut App, event: &KeyEvent, tx: &mut Tx) -> bool {
    // The command bar takes the keys its field leaves over: the arrows pick a row, Return runs
    // it, Escape closes it, and its own chord closes it too.
    if s.palette.is_some() {
        if matches!(event.key, Key::Escape) {
            s.close_palette();
            return true;
        }
        if s.keybinds.matches(Action::CommandBar, event) {
            s.close_palette();
            return true;
        }
        match event.key {
            Key::Down => s.palette_move(1),
            Key::Up => s.palette_move(-1),
            Key::Enter => s.run_palette_command(tx),
            _ => return false,
        }
        return true;
    }
    // The sheet, the row menu and the settings own the keyboard while open: their fields and
    // rows handle Return, Escape, the arrows and the focus trap.
    if s.sheet.is_some() || s.menu.is_some() || s.settings.is_some() {
        return false;
    }
    let mods = event.mods;
    let typing = tx.focused().is_some_and(|key| key.ends_with("input"));
    // The commands that change files wait until no field has the keyboard: a shortcut like F2,
    // Cmd+C or Delete should type in the search field, not touch what is selected.
    let guarded = [
        Action::Rename,
        Action::Move,
        Action::Trash,
        Action::Copy,
        Action::Cut,
        Action::Paste,
        Action::Duplicate,
    ];
    for action in Action::ALL {
        if typing && guarded.contains(&action) {
            continue;
        }
        if s.keybinds.matches(action, event) {
            s.perform(action, tx);
            return true;
        }
    }
    // Backspace alone still goes to the enclosing folder, a habit the bindings don't cover.
    if matches!(event.key, Key::Backspace) && !mods.cmd && !mods.ctrl && !mods.alt {
        s.up();
        return true;
    }
    if matches!(event.key, Key::Escape) {
        // As in the table itself, Escape takes a selection of several back to one row first.
        if s.selection_len() > 1 {
            s.collapse_selection();
            return true;
        }
        if !s.search.text.is_empty() {
            s.search.clear();
            s.apply_filter();
            return true;
        }
    }
    // A focused field owns its keys; the list's own handler already saw the ones it takes.
    if tx.focused().is_some() {
        return false;
    }
    // The forward-delete key trashes the selection, as the Delete key does everywhere.
    if matches!(event.key, Key::Delete) && !mods.cmd && !mods.ctrl && !mods.alt {
        if s.selection_len() > 0 {
            s.begin_trash();
            return true;
        }
        return false;
    }
    // Typing a name jumps to it, as it does in every file list.
    if let Key::Text(text) = &event.key
        && !text.is_empty()
        && !mods.cmd
        && !mods.ctrl
        && !mods.alt
        && s.typeahead_select(text, tx)
    {
        return true;
    }
    list_key(s, event, tx)
}

/// The keys the file list owns: arrows, paging, Home/End, Return, Cmd/Ctrl+A and Escape.
pub fn list_key(s: &mut App, event: &KeyEvent, tx: &mut Tx) -> bool {
    let mods = event.mods;
    // Cmd/Ctrl+A selects every row; a chord holding Alt is left to the bindings.
    if event.is_command('a') && !mods.alt {
        s.select_all();
        return true;
    }
    if matches!(event.key, Key::Escape) {
        if s.selection_len() > 1 {
            s.collapse_selection();
            return true;
        }
        return false;
    }
    // Cmd/Ctrl+arrow and Alt+arrow belong to the bindings (enclosing folder) or nothing.
    if mods.cmd || mods.ctrl || mods.alt {
        return false;
    }
    match event.key {
        Key::Up => s.move_selection(-1, mods, tx),
        Key::Down => s.move_selection(1, mods, tx),
        Key::PageUp => s.move_selection(-10, mods, tx),
        Key::PageDown => s.move_selection(10, mods, tx),
        Key::Home => s.select_edge(false, mods, tx),
        Key::End => s.select_edge(true, mods, tx),
        Key::Enter => {
            if s.selected.is_none() || tx.key_repeat() {
                return false;
            }
            s.open_selected();
        }
        _ => return false,
    }
    true
}
