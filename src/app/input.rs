use crate::app::App;
use crate::keybinds::Action;
use rune::{Key, KeyEvent, Tx};

pub fn on_key(s: &mut App, event: &KeyEvent, tx: &mut Tx) -> bool {
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
            Key::Down => s.palette_move(1, tx),
            Key::Up => s.palette_move(-1, tx),
            Key::Enter => s.run_palette_command(tx),
            _ => return false,
        }
        return true;
    }
    if s.rule_picker.is_some() {
        match event.key {
            Key::Escape => s.close_rule_picker(),
            Key::Down => s.rule_picker_move(1),
            Key::Up => s.rule_picker_move(-1),
            Key::Enter => s.confirm_rule_picker(),
            _ => return false,
        }
        return true;
    }
    if s.sheet.is_some()
        || s.menu.is_some()
        || s.empty_menu.is_some()
        || s.header_menu.is_some()
        || s.place_menu.is_some()
        || s.settings.is_some()
    {
        return false;
    }
    if s.place_rename.is_some() {
        return false;
    }
    // A shell with the keyboard keeps every key, Ctrl-C and Escape included. Only the pane's own
    // chords and the commands that leave the pane reach the app while it has focus.
    if s.term.is_open() && s.term.focus {
        if s.terminal_chord(event, tx) {
            return true;
        }
        for action in [
            Action::CommandBar,
            Action::OpenSettings,
            Action::OpenTerminal,
        ] {
            if s.keybinds.matches(action, event) {
                s.perform(action, tx);
                return true;
            }
        }
        if s.term.key(event) {
            return true;
        }
    }
    let mods = event.mods;
    let typing = tx.focused().is_some_and(|key| key.ends_with("input"));
    let guarded = [
        Action::FocusSearch,
        Action::Rename,
        Action::Move,
        Action::Trash,
        Action::Delete,
        Action::Copy,
        Action::Cut,
        Action::Paste,
        Action::Duplicate,
        Action::SelectAll,
        Action::DeselectAll,
        Action::InvertSelection,
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
    // Backspace alone goes to the enclosing folder, a habit the bindings don't cover.
    if matches!(event.key, Key::Backspace) && !mods.cmd && !mods.ctrl && !mods.alt {
        s.up();
        return true;
    }
    if matches!(event.key, Key::Escape) {
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
    if tx.focused().is_some() {
        return false;
    }
    if matches!(event.key, Key::Delete) && !mods.cmd && !mods.ctrl && !mods.alt {
        if s.selection_len() > 0 {
            s.begin_trash();
            return true;
        }
        return false;
    }
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

pub fn list_key(s: &mut App, event: &KeyEvent, tx: &mut Tx) -> bool {
    let mods = event.mods;
    if matches!(event.key, Key::Escape) {
        if s.selection_len() > 1 {
            s.collapse_selection();
            return true;
        }
        return false;
    }
    if mods.cmd || mods.ctrl || mods.alt {
        return false;
    }
    let columns = s.grid_columns() as isize;
    let step = if s.is_grid() { columns } else { 1 };
    let page = step * 10;
    match event.key {
        Key::Up => s.move_selection(-step, mods, tx),
        Key::Down => s.move_selection(step, mods, tx),
        Key::Left if s.is_grid() => s.move_selection(-1, mods, tx),
        Key::Right if s.is_grid() => s.move_selection(1, mods, tx),
        Key::PageUp => s.move_selection(-page, mods, tx),
        Key::PageDown => s.move_selection(page, mods, tx),
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
