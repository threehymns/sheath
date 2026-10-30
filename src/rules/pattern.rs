//! The patterns a rule's matchers name: glob text, whole paths with `~`, and how the config
//! writes a string back out.

/// `text` the way KDL writes a string argument: quoted and escaped when it has to be.
pub fn quote(text: &str) -> String {
    kdl::KdlValue::String(text.to_owned()).to_string()
}

/// Whether `pattern` matches all of `text`. `*` stops at a separator, `**` crosses them, `?` is
/// one character, and everything else is literal.
pub fn glob(pattern: &str, text: &str) -> bool {
    glob_bytes(pattern.as_bytes(), text.as_bytes())
}

/// A byte that ends one part of a path, whichever platform wrote it: a rule from one machine has
/// to match the same folder on another, and Windows paths carry both characters.
fn is_separator(byte: u8) -> bool {
    byte == b'/' || byte == b'\\'
}

fn glob_bytes(pattern: &[u8], text: &[u8]) -> bool {
    let Some(&first) = pattern.first() else {
        return text.is_empty();
    };
    match first {
        b'*' if pattern.get(1) == Some(&b'*') => {
            let mut rest = &pattern[2..];
            while rest.first() == Some(&b'*') {
                rest = &rest[1..];
            }
            if rest.is_empty() {
                return true;
            }
            (0..=text.len()).any(|at| glob_bytes(rest, &text[at..]))
        }
        b'*' => {
            let rest = &pattern[1..];
            if rest.is_empty() {
                return !text.iter().any(|&byte| is_separator(byte));
            }
            for at in 0..=text.len() {
                if text[..at].iter().any(|&byte| is_separator(byte)) {
                    break;
                }
                if glob_bytes(rest, &text[at..]) {
                    return true;
                }
            }
            false
        }
        b'?' => !text.is_empty() && !is_separator(text[0]) && glob_bytes(&pattern[1..], &text[1..]),
        literal => !text.is_empty() && text[0] == literal && glob_bytes(&pattern[1..], &text[1..]),
    }
}

/// Whether a path pattern matches `path`, with `~` read as home, `/` and `\` read the same, and
/// a trailing `/**` also matching the folder itself.
pub fn path_match(pattern: &str, path: &str) -> bool {
    let pattern = expand_home(pattern).replace('\\', "/");
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    let path = if path.is_empty() { "/" } else { path };
    if glob(&pattern, path) {
        return true;
    }
    match pattern.strip_suffix("/**") {
        Some(base) => glob(base, path),
        None => false,
    }
}

/// Replaces a leading `~` with the user's home directory.
pub fn expand_home(pattern: &str) -> String {
    let Some(rest) = pattern.strip_prefix('~') else {
        return pattern.to_owned();
    };
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return pattern.to_owned();
    };
    let home = home.to_string_lossy();
    format!("{}{rest}", home.trim_end_matches(['/', '\\']))
}
