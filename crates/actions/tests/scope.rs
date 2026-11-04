//! Fails when actions grows past its rule: `rune` only, no files, no widgets, no app.

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const FORBIDDEN: &[&str] = &[
    "actions_ui::",
    "actions_kdl::",
    "rune_kit::",
    "rune_tty::",
    "scroll::",
    "std::fs",
    "kdl::",
    "notify::",
];

#[test]
fn depends_on_rune_only() {
    let deps = MANIFEST
        .split("[dependencies]")
        .nth(1)
        .expect("the manifest has a dependencies section")
        .split("\n[")
        .next()
        .unwrap();
    for line in deps.lines().map(str::trim).filter(|l| !l.is_empty()) {
        assert!(
            line.starts_with("rune.workspace") || line.starts_with('#'),
            "actions may depend on rune only, found: {line}"
        );
    }
}

#[test]
fn owns_no_files_and_no_widgets() {
    for (name, source) in sources() {
        for path in FORBIDDEN {
            assert!(
                !source.contains(path),
                "{name} mentions `{path}`; files and widgets belong to the host"
            );
        }
    }
}

#[test]
fn readme_states_the_scope() {
    assert!(
        README.contains("## Scope"),
        "the README keeps its scope rule"
    );
}

fn sources() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            out.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    out
}
