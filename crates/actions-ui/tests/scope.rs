//! Fails when actions-ui grows past its rule: no files, no app types, no sibling crates.

const MANIFEST: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");
const FORBIDDEN: &[&str] = &[
    "std::fs",
    "notify::",
    "kdl::",
    "chrome::",
    "scroll::",
    "rune_tty::",
];

#[test]
fn depends_on_the_kernel_and_the_framework() {
    let deps = MANIFEST
        .split("[dependencies]")
        .nth(1)
        .expect("the manifest has a dependencies section")
        .split("\n[")
        .next()
        .unwrap();
    for line in deps.lines().map(str::trim).filter(|l| !l.is_empty()) {
        assert!(
            line.starts_with("actions.workspace")
                || line.starts_with("rune.workspace")
                || line.starts_with("rune-kit.workspace"),
            "actions-ui may depend on actions, rune and rune-kit only, found: {line}"
        );
    }
}

#[test]
fn owns_no_files_and_no_app_types() {
    for (name, source) in sources() {
        for path in FORBIDDEN {
            assert!(
                !source.contains(path),
                "{name} mentions `{path}`; the app owns files and composition"
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
