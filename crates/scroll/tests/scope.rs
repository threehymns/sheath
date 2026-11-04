//! Fails when scroll grows past its rule: `rune` only, no files, no sibling crate's paths.

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const FORBIDDEN: &[&str] = &[
    "actions::",
    "actions_ui::",
    "rune_kit::",
    "rune_tty::",
    "std::fs",
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
            "scroll may depend on rune only, found: {line}"
        );
    }
}

#[test]
fn owns_no_files_and_no_sibling_crate() {
    for (name, source) in sources() {
        for path in FORBIDDEN {
            assert!(
                !source.contains(path),
                "{name} mentions `{path}`; physics knows nothing about files or other crates"
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
