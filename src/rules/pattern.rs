//! The patterns a rule's matchers name: glob text, whole paths with `~`, and how the config
//! writes a string back out.

/// `text` the way KDL writes a string argument: quoted and escaped when it has to be.
pub fn quote(text: &str) -> String {
    kdl::KdlValue::String(text.to_owned()).to_string()
}

/// Whether `pattern` matches all of `text`, with gitignore's reading of `**`: a run of two
/// asterisks standing alone between separators crosses any number of segments, none included,
/// and every other asterisk is a plain `*`, which stops at a separator. `?` is one character and
/// everything else is literal.
pub fn glob(pattern: &str, text: &str) -> bool {
    glob_bytes(pattern.as_bytes(), text.as_bytes(), true)
}

/// A byte that ends one part of a path, whichever platform wrote it: a rule from one machine has
/// to match the same folder on another, and Windows paths carry both characters.
fn is_separator(byte: u8) -> bool {
    byte == b'/' || byte == b'\\'
}

/// `at_boundary` is whether the pattern stands at the start of a segment, which is the only
/// place a `**` crosses them. Consuming a separator enters the next segment, and that is what
/// makes `a/**/b` read as "any number of segments, then `b`".
fn glob_bytes(pattern: &[u8], text: &[u8], at_boundary: bool) -> bool {
    let Some(&first) = pattern.first() else {
        return text.is_empty();
    };
    match first {
        b'*' => {
            let run = pattern.iter().take_while(|&&byte| byte == b'*').count();
            let rest = &pattern[run..];
            // A `**` that is a whole path component means any number of segments. A run of any
            // other length, and one inside a name, is a plain `*`, as gitignore reads it.
            let whole = at_boundary && run == 2 && rest.first().is_none_or(|&b| is_separator(b));
            if whole {
                // `**/` may cross no segments at all, so the separator after it is optional:
                // `a/**/b` matches `a/b` as well as `a/x/y/b`.
                if rest.first() == Some(&b'/') && glob_bytes(&rest[1..], text, true) {
                    return true;
                }
                if rest.is_empty() {
                    return true;
                }
                return (0..=text.len()).any(|at| glob_bytes(rest, &text[at..], false));
            }
            if rest.is_empty() {
                return !text.iter().any(|&byte| is_separator(byte));
            }
            for at in 0..=text.len() {
                if text[..at].iter().any(|&byte| is_separator(byte)) {
                    break;
                }
                if glob_bytes(rest, &text[at..], false) {
                    return true;
                }
            }
            false
        }
        b'?' => {
            !text.is_empty()
                && !is_separator(text[0])
                && glob_bytes(&pattern[1..], &text[1..], false)
        }
        separator if is_separator(separator) => {
            text.first() == Some(&separator) && glob_bytes(&pattern[1..], &text[1..], true)
        }
        literal => {
            !text.is_empty() && text[0] == literal && glob_bytes(&pattern[1..], &text[1..], false)
        }
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
