//! The kernel's contract, driven through a stand-in app enum: no window, no files.

use actions::{Action, Keybinds};
use rune::{Key, KeyEvent, Mods, Shortcut, ShortcutError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Test {
    Rename,
    Copy,
    Unbound,
}

impl Action for Test {
    const ALL: &'static [Self] = &[Test::Rename, Test::Copy, Test::Unbound];

    fn from_id(id: &str) -> Option<Test> {
        Test::ALL.iter().copied().find(|action| action.id() == id)
    }

    fn id(self) -> &'static str {
        match self {
            Test::Rename => "rename",
            Test::Copy => "copy",
            Test::Unbound => "unbound",
        }
    }

    fn label(self) -> &'static str {
        self.id()
    }

    fn default_chord(self) -> Option<&'static str> {
        match self {
            Test::Rename => Some("f2"),
            Test::Copy => Some("ctrl+c"),
            Test::Unbound => None,
        }
    }
}

fn chord(text: &str) -> Shortcut {
    text.parse().expect("the test's chord parses")
}

fn press(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods::default(),
    }
}

#[test]
fn every_action_round_trips_through_its_id() {
    for action in Test::ALL {
        assert_eq!(Test::from_id(action.id()), Some(*action));
    }
    assert_eq!(Test::from_id("missing"), None);
}

#[test]
fn defaults_are_bound_and_an_unbound_action_has_no_chord() {
    let binds = Keybinds::<Test>::defaults();
    assert_eq!(binds.chord(Test::Rename), Some(&chord("f2")));
    assert_eq!(binds.chord(Test::Copy), Some(&chord("ctrl+c")));
    assert_eq!(binds.chord(Test::Unbound), None);
    assert!(!binds.is_custom(Test::Rename));
}

#[test]
fn a_chord_taken_by_another_action_is_a_conflict() {
    let mut binds = Keybinds::<Test>::defaults();
    assert!(
        matches!(
            binds.set(Test::Rename, Some(&chord("ctrl+c"))),
            Err(ShortcutError::Conflict)
        ),
        "Copy already holds Ctrl+C"
    );
    assert!(binds.set(Test::Rename, Some(&chord("ctrl+r"))).is_ok());
    assert!(binds.is_custom(Test::Rename));
}

#[test]
fn reset_restores_the_default_and_clearing_unbinds() {
    let mut binds = Keybinds::<Test>::defaults();
    binds
        .set(Test::Copy, None)
        .expect("clearing always succeeds");
    assert_eq!(binds.chord(Test::Copy), None);
    binds.reset(Test::Copy);
    assert_eq!(binds.chord(Test::Copy), Some(&chord("ctrl+c")));

    binds
        .set(Test::Rename, Some(&chord("ctrl+r")))
        .expect("Ctrl+R is free");
    assert!(binds.is_custom(Test::Rename));
    binds.reset(Test::Rename);
    assert!(!binds.is_custom(Test::Rename));
}

#[test]
fn entries_apply_over_the_defaults() {
    let binds =
        Keybinds::<Test>::from_entries([(Test::Rename, Some(chord("f3"))), (Test::Copy, None)]);
    assert_eq!(binds.chord(Test::Rename), Some(&chord("f3")));
    assert_eq!(binds.chord(Test::Copy), None);
    assert_eq!(binds.chord(Test::Unbound), None);
}

#[test]
fn matches_reads_the_physical_key() {
    let binds = Keybinds::<Test>::defaults();
    assert!(binds.matches(Test::Rename, &press(Key::Function(2))));
    assert!(!binds.matches(Test::Rename, &press(Key::Function(3))));
    assert!(!binds.matches(Test::Copy, &press(Key::Function(2))));
}

#[test]
fn keycaps_and_menu_labels_come_from_the_chord() {
    let binds = Keybinds::<Test>::defaults();
    assert!(!binds.keycaps(Test::Rename).is_empty());
    assert!(binds.keycaps(Test::Unbound).is_empty());
    assert_eq!(binds.menu_label(Test::Unbound), "");
}
