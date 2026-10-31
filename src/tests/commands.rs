//! The command bar.

use super::helpers::*;
use crate::app::{SettingsTab, SheetKind};
use crate::keybinds::Action;
use rune::{Key, KeyEvent, Mods};

#[test]
fn ctrl_p_and_ctrl_shift_p_open_the_command_bar() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    let open = command_bar_event(&rt.state, false);
    rt.key(open.clone());
    rt.frame(0.1);
    assert!(rt.state.palette.is_some(), "Ctrl+P opens it");
    assert!(rt.find("command-bar").is_some());
    assert!(
        rt.find("command-rename").is_some(),
        "the registry is listed"
    );

    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.palette.is_none(), "Escape closes it");

    rt.key(command_bar_event(&rt.state, true));
    rt.frame(0.1);
    assert!(rt.state.palette.is_some(), "Ctrl+Shift+P opens it too");

    rt.key(open);
    rt.frame(0.1);
    assert!(rt.state.palette.is_none(), "its chord closes it again");
}

#[test]
fn the_command_bar_filters_and_runs_a_command() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.select_id("f:notes.md");

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Text("rena".into()),
        mods: Mods::default(),
    });
    rt.frame(0.1);

    assert!(rt.find("command-rename").is_some());
    assert!(
        rt.find("command-refresh").is_none(),
        "the query filters the other commands out"
    );

    rt.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert!(rt.state.palette.is_none());
    assert_eq!(
        rt.state.sheet.as_ref().map(|s| s.kind),
        Some(SheetKind::Rename),
        "Return ran the filtered command"
    );
}

#[test]
fn the_command_bar_runs_a_command_picked_with_the_pointer() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    click(&mut rt, "command-new-folder");

    assert!(rt.state.palette.is_none(), "picking one closes the bar");
    assert!(dir.path().join("New Folder").is_dir());
}

#[test]
fn the_command_bar_moves_the_selection_with_the_arrows() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    assert_eq!(rt.state.palette.as_ref().map(|p| p.selected), Some(0));

    rt.key(KeyEvent {
        key: Key::Down,
        mods: Mods::default(),
    });
    rt.frame(0.1);
    assert_eq!(rt.state.palette.as_ref().map(|p| p.selected), Some(1));
    rt.key(KeyEvent {
        key: Key::Up,
        mods: Mods::default(),
    });
    rt.frame(0.1);

    let (x, y) = rt
        .find("command-open-settings")
        .expect("Settings is listed");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert_eq!(rt.state.settings, Some(SettingsTab::General));
}

#[test]
fn the_command_bar_leaves_commands_that_need_a_selection_alone() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Text("trash".into()),
        mods: Mods::default(),
    });
    rt.frame(0.1);
    rt.key(KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    rt.frame(0.1);

    assert!(rt.state.sheet.is_none(), "nothing is selected to trash");
    assert!(rt.state.palette.is_none());
}

#[test]
fn the_command_bar_rows_keep_their_padding_from_the_edges() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.state.open_palette();
    rt.invalidate();
    rt.frame(0.1);

    let ([bar_x, _, bar_w, _], _) = rt.presented("command-bar").expect("the bar is drawn");
    let (row_x, row_w) = {
        let ([x, _, w, _], _) = rt
            .presented("command-rename")
            .expect("a command row is drawn");
        (x, w)
    };
    assert!(row_x >= bar_x + 5.0, "the row starts inside the bar");
    assert!(
        row_x + row_w <= bar_x + bar_w - 5.0,
        "and stops short of the far edge instead of clipping"
    );
}

#[test]
fn the_arrows_scroll_the_bar_and_keep_rows_around_the_highlight() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.state.open_palette();
    rt.invalidate();
    rt.frame(0.1);

    let actions = rt.state.palette_actions();
    let down = KeyEvent {
        key: Key::Down,
        mods: Mods::default(),
    };
    // Enough presses to take the highlight past the fold of the 380-point list.
    for _ in 0..10 {
        rt.key(down.clone());
        rt.frame(0.1);
    }
    // The reveal glides, so let it come to rest.
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }

    let selected = rt.state.palette.as_ref().expect("the bar is open").selected;
    assert_eq!(selected, 10, "the presses moved the highlight with it");
    let row = rt
        .presented(&format!("command-{}", actions[selected].id()))
        .expect("the highlighted row is drawn")
        .0;
    let viewport = rt.presented("command-scroll").expect("the list is drawn").0;
    let bottom = viewport[1] + viewport[3];
    assert!(
        row[1] >= viewport[1] - 0.5 && row[1] + row[3] <= bottom + 0.5,
        "the highlight is inside the list: {row:?} in {viewport:?}"
    );
    assert!(
        bottom - (row[1] + row[3]) >= 2.0 * 33.0,
        "and a few rows stay below it: {row:?} in {viewport:?}"
    );
}

#[test]
fn the_command_bar_keys_follow_the_highlighted_row() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.state.open_palette();
    rt.invalidate();
    rt.frame(0.1);

    let actions = rt.state.palette_actions();
    let selected = rt.state.palette.as_ref().expect("the bar is open").selected;
    let well = |action: Action| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&format!("keys-{}", action.id())))
            .map(|e| e.fill)
    };
    let lit = well(actions[selected]).expect("the selected row has a tray");
    let dim = actions
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| *index != selected)
        .find_map(|(_, action)| well(action))
        .expect("another row has a tray");
    assert_ne!(lit, dim, "the well is mixed with the row it sits on");
}

#[test]
fn the_command_bar_draws_shortcuts_as_keycaps() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    rt.state.open_palette();
    rt.invalidate();
    rt.frame(0.1);

    // A keycap is a legend on a face, sitting in the chord's tray.
    let first = rt.state.keybinds.keycaps(Action::Rename)[0].clone();
    let drawn = rt.elements().iter().any(|e| {
        e.path.contains("/command-rename/")
            && e.path.ends_with("/keycap0/cap/face/label")
            && e.text.as_deref() == Some(first.as_str())
    });
    assert!(
        drawn,
        "the rename row draws its chord {first:?} on a key in a tray"
    );
}
