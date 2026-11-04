//! The codec's contract: read blocks, write one key, leave the rest of the document alone.

use actions::{Action, Keybinds, Setting};
use kdl::KdlDocument;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Test {
    Rename,
    Copy,
}

impl Action for Test {
    const ALL: &'static [Self] = &[Test::Rename, Test::Copy];

    fn from_id(id: &str) -> Option<Test> {
        Test::ALL.iter().copied().find(|action| action.id() == id)
    }

    fn id(self) -> &'static str {
        match self {
            Test::Rename => "rename",
            Test::Copy => "copy",
        }
    }

    fn label(self) -> &'static str {
        self.id()
    }

    fn default_chord(self) -> Option<&'static str> {
        match self {
            Test::Rename => Some("f2"),
            Test::Copy => Some("ctrl+c"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Flag {
    Grid,
}

impl Setting for Flag {
    const ALL: &'static [Self] = &[Flag::Grid];

    fn from_name(name: &str) -> Option<Flag> {
        (name == "grid").then_some(Flag::Grid)
    }

    fn name(self) -> &'static str {
        "grid"
    }

    fn id(self) -> &'static str {
        "grid"
    }
}

fn document() -> KdlDocument {
    r#"
// keep this comment
keybinds {
    rename "f3"
    copy ""
}

other {
    keep 1
}
"#
    .parse()
    .expect("the fixture parses")
}

#[test]
fn a_document_reads_over_the_defaults() {
    let doc = document();
    let binds = actions_kdl::keybinds::read::<Test>(Some(&doc));
    assert_eq!(binds.accelerator(Test::Rename), "f3");
    assert_eq!(binds.chord(Test::Copy), None, "an empty chord clears it");
}

#[test]
fn a_missing_document_is_the_defaults() {
    let binds = actions_kdl::keybinds::read::<Test>(None);
    assert_eq!(binds.accelerator(Test::Rename), "f2");
}

#[test]
fn unknown_actions_and_broken_chords_leave_the_default_alone() {
    let doc: KdlDocument = "keybinds {\n    missing \"f4\"\n    rename \"f3x\"\n}"
        .parse()
        .expect("the fixture parses");
    let binds = actions_kdl::keybinds::read::<Test>(Some(&doc));
    assert_eq!(binds.accelerator(Test::Rename), "f2");
}

#[test]
fn writing_one_chord_keeps_comments_and_other_nodes() {
    let mut doc = document();
    let mut binds = Keybinds::<Test>::defaults();
    binds
        .set(
            Test::Copy,
            Some(&"ctrl+x".parse().expect("the chord parses")),
        )
        .expect("Ctrl+X is free");
    actions_kdl::keybinds::set(&mut doc, &binds, Test::Copy);

    let text = doc.to_string();
    assert!(text.contains("copy \"ctrl+x\""), "the new chord is written");
    assert!(text.contains("// keep this comment"), "comments survive");
    assert!(text.contains("keep 1"), "unrelated nodes survive");
}

#[test]
fn a_default_chord_is_removed_instead_of_written_down() {
    let mut doc = document();
    let binds = Keybinds::<Test>::defaults();
    actions_kdl::keybinds::set(&mut doc, &binds, Test::Rename);

    let text = doc.to_string();
    assert!(!text.contains("rename"), "the override is taken out");
    assert!(text.contains("// keep this comment"));
}

#[test]
fn clearing_writes_an_empty_chord() {
    let mut doc = document();
    let mut binds = Keybinds::<Test>::defaults();
    binds.set(Test::Rename, None).expect("clearing succeeds");
    actions_kdl::keybinds::set(&mut doc, &binds, Test::Rename);
    assert!(doc.to_string().contains("rename \"\""));
}

#[test]
fn a_setting_parses_writes_and_removes_in_place() {
    let doc: KdlDocument = "settings {\n    grid #true\n}"
        .parse()
        .expect("the fixture parses");
    assert_eq!(
        actions_kdl::settings::parse::<Flag>(&doc),
        vec![(Flag::Grid, true)]
    );

    let mut doc: KdlDocument = "// keep\nsettings {\n    grid #true\n}\n"
        .parse()
        .expect("the fixture parses");
    actions_kdl::settings::write(&mut doc, Flag::Grid, false);
    assert!(doc.to_string().contains("grid #false"));
    assert!(doc.to_string().contains("// keep"));

    actions_kdl::settings::remove(&mut doc, Flag::Grid);
    assert!(!doc.to_string().contains("grid"));
    assert!(doc.to_string().contains("// keep"));
}
