//! Folder and entry rules: the nodes in `config.kdl`, the glob matchers, and the view a folder
//! resolves to. Everything here is pure: it takes a parsed document and a path, and returns
//! what the browser should show.

use crate::app::ViewMode;
use kdl::{KdlDocument, KdlNode};
use rune_kit::SortDir;
use std::path::Path;

/// Which listing a rule governs: a folder's own presentation, or one entry's presence in the
/// folder that lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Folder,
    Entry,
}

/// What an entry is, for `kind="folder"` and `kind="file"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Folder,
}

/// The orderings a folder rule can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
}

impl SortKey {
    pub fn as_str(self) -> &'static str {
        match self {
            SortKey::Name => "name",
            SortKey::Size => "size",
            SortKey::Modified => "modified",
        }
    }

    fn parse(value: &str) -> Option<SortKey> {
        match value {
            "name" => Some(SortKey::Name),
            "size" => Some(SortKey::Size),
            "modified" => Some(SortKey::Modified),
            _ => None,
        }
    }
}

/// One `match` or `exclude` directive: every field it names must hold for the entry or folder
/// to count as matched.
#[derive(Clone, Debug, Default)]
pub struct Matcher {
    pub path: Option<String>,
    pub name: Option<String>,
    pub kind: Option<EntryKind>,
    pub extension: Option<String>,
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
}

/// One entry as a matcher sees it.
pub struct EntryRef<'a> {
    pub path: &'a str,
    pub name: &'a str,
    pub is_dir: bool,
}

impl EntryRef<'_> {
    fn extension(&self) -> &str {
        self.name
            .rsplit_once('.')
            .map(|(_, extension)| extension)
            .unwrap_or("")
    }
}

impl Matcher {
    fn matches_folder(&self, path: &str, count: usize) -> bool {
        self.path
            .as_ref()
            .is_none_or(|pattern| path_match(pattern, path))
            && self.min_items.is_none_or(|min| count >= min)
            && self.max_items.is_none_or(|max| count <= max)
    }

    fn matches_entry(&self, entry: &EntryRef<'_>) -> bool {
        self.path
            .as_ref()
            .is_none_or(|pattern| path_match(pattern, entry.path))
            && self.name.as_ref().is_none_or(|pattern| glob(pattern, entry.name))
            && self
                .extension
                .as_ref()
                .is_none_or(|pattern| glob(pattern, entry.extension()))
            && self.kind.is_none_or(|kind| {
                (kind == EntryKind::Folder) == entry.is_dir
            })
    }

    /// The directive as the Rules tab prints it.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(path) = &self.path {
            parts.push(format!("path={path}"));
        }
        if let Some(name) = &self.name {
            parts.push(format!("name={name}"));
        }
        if let Some(kind) = self.kind {
            parts.push(format!(
                "kind={}",
                match kind {
                    EntryKind::File => "file",
                    EntryKind::Folder => "folder",
                }
            ));
        }
        if let Some(extension) = &self.extension {
            parts.push(format!("extension={extension}"));
        }
        if let Some(min) = self.min_items {
            parts.push(format!("min-items={min}"));
        }
        if let Some(max) = self.max_items {
            parts.push(format!("max-items={max}"));
        }
        parts.join(" ")
    }
}

/// A rule property: absent, set to a value, or set to `inherit` to undo an earlier rule's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop<T> {
    Unset,
    Inherit,
    Set(T),
}

impl<T> Default for Prop<T> {
    fn default() -> Self {
        Prop::Unset
    }
}

/// What a rule sets. A folder rule reads the first four; an entry rule reads `hidden`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Props {
    pub view: Prop<ViewMode>,
    pub sort: Prop<SortKey>,
    pub direction: Prop<SortDir>,
    pub show_hidden: Prop<bool>,
    pub hidden: Prop<bool>,
}

/// One `folder-rule` or `entry-rule`, in file order.
#[derive(Clone, Debug)]
pub struct Rule {
    pub name: Option<String>,
    pub kind: Kind,
    pub matches: Vec<Matcher>,
    pub excludes: Vec<Matcher>,
    pub props: Props,
}

impl Rule {
    fn applies_to_folder(&self, path: &str, count: usize) -> bool {
        if self.kind != Kind::Folder {
            return false;
        }
        let matched = self.matches.is_empty() || self.matches.iter().any(|m| m.matches_folder(path, count));
        matched && !self.excludes.iter().any(|m| m.matches_folder(path, count))
    }

    fn applies_to_entry(&self, entry: &EntryRef<'_>) -> bool {
        if self.kind != Kind::Entry {
            return false;
        }
        let matched = self.matches.is_empty() || self.matches.iter().any(|m| m.matches_entry(entry));
        matched && !self.excludes.iter().any(|m| m.matches_entry(entry))
    }

    /// The rule as the Rules tab names it: the name it was given, or its first matcher.
    pub fn summary(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
        let first = self
            .matches
            .first()
            .map(Matcher::summary)
            .unwrap_or_default();
        if first.is_empty() {
            "everything".to_owned()
        } else {
            first
        }
    }

    /// The kind as the tab labels it.
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            Kind::Folder => "Folder",
            Kind::Entry => "Entry",
        }
    }
}

/// Every rule, in the order the file holds them.
#[derive(Clone, Debug, Default)]
pub struct Rules {
    pub rules: Vec<Rule>,
}

/// The presentation a folder resolves to. `None` means no rule spoke, so the remembered or
/// global value applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    pub view: Option<ViewMode>,
    pub sort: Option<SortKey>,
    pub direction: Option<SortDir>,
    pub show_hidden: Option<bool>,
}

impl Resolved {
    /// Fills what this resolution leaves unset from `fallback`.
    pub fn or(self, fallback: Resolved) -> Resolved {
        Resolved {
            view: self.view.or(fallback.view),
            sort: self.sort.or(fallback.sort),
            direction: self.direction.or(fallback.direction),
            show_hidden: self.show_hidden.or(fallback.show_hidden),
        }
    }
}

fn merge<T: Copy>(slot: &mut Option<T>, prop: Prop<T>) {
    match prop {
        Prop::Set(value) => *slot = Some(value),
        Prop::Inherit => *slot = None,
        Prop::Unset => {}
    }
}

impl Rules {
    /// Reads every `folder-rule` and `entry-rule` in the document, in file order. A property or
    /// matcher the schema does not know is an error: the caller keeps its last good rules and
    /// says why.
    pub fn parse(doc: &KdlDocument) -> Result<Rules, String> {
        let mut rules = Vec::new();
        for node in doc.nodes() {
            let kind = match node.name().value() {
                "folder-rule" => Kind::Folder,
                "entry-rule" => Kind::Entry,
                _ => continue,
            };
            rules.push(parse_rule(node, kind)?);
        }
        Ok(Rules { rules })
    }

    pub fn resolve(&self, path: &Path, count: usize) -> Resolved {
        let path = path.to_string_lossy();
        let mut resolved = Resolved::default();
        for rule in &self.rules {
            if !rule.applies_to_folder(&path, count) {
                continue;
            }
            merge(&mut resolved.view, rule.props.view);
            merge(&mut resolved.sort, rule.props.sort);
            merge(&mut resolved.direction, rule.props.direction);
            merge(&mut resolved.show_hidden, rule.props.show_hidden);
        }
        resolved
    }

    /// The first rule matching a folder, in file order, for the command that opens the tab on
    /// the rule controlling what is on screen.
    pub fn first_folder(&self, path: &Path, count: usize) -> Option<usize> {
        let path = path.to_string_lossy();
        self.rules
            .iter()
            .position(|rule| rule.applies_to_folder(&path, count))
    }

    /// The first rule matching an entry, in file order.
    pub fn first_entry(&self, entry: &EntryRef<'_>) -> Option<usize> {
        self.rules
            .iter()
            .position(|rule| rule.applies_to_entry(entry))
    }

    /// Whether a rule hides or reveals `entry`, after the folder's own hidden policy has had its
    /// say. `None` leaves the folder's decision alone.
    pub fn hidden(&self, entry: &EntryRef<'_>) -> Option<bool> {
        let mut hidden = None;
        for rule in &self.rules {
            if rule.applies_to_entry(entry) {
                merge(&mut hidden, rule.props.hidden);
            }
        }
        hidden
    }
}

fn parse_rule(node: &KdlNode, kind: Kind) -> Result<Rule, String> {
    let name = node.get(0).and_then(|entry| entry.as_string()).map(str::to_owned);
    let mut rule = Rule {
        name,
        kind,
        matches: Vec::new(),
        excludes: Vec::new(),
        props: Props::default(),
    };
    let Some(children) = node.children() else {
        return Ok(rule);
    };
    for child in children.nodes() {
        match child.name().value() {
            "match" => rule.matches.push(parse_matcher(child, kind)?),
            "exclude" => rule.excludes.push(parse_matcher(child, kind)?),
            "view" => rule.props.view = parse_prop(child, "view", Kind::Folder, kind, parse_view)?,
            "sort" => rule.props.sort = parse_prop(child, "sort", Kind::Folder, kind, SortKey::parse)?,
            "direction" => {
                rule.props.direction = parse_prop(child, "direction", Kind::Folder, kind, parse_direction)?
            }
            "show-hidden" => {
                rule.props.show_hidden =
                    parse_prop(child, "show-hidden", Kind::Folder, kind, parse_bool)?
            }
            "hidden" => {
                rule.props.hidden = parse_prop(child, "hidden", Kind::Entry, kind, parse_bool)?
            }
            other => {
                return Err(format!(
                    "unknown property `{other}`; folder rules and entry rules support view, \
                     sort, direction, show-hidden, and hidden"
                ));
            }
        }
    }
    Ok(rule)
}

fn parse_prop<T: Copy>(
    node: &KdlNode,
    name: &str,
    allowed: Kind,
    on: Kind,
    parse: fn(&str) -> Option<T>,
) -> Result<Prop<T>, String> {
    if allowed != on {
        return Err(format!(
            "`{name}` is only for {} rules",
            match allowed {
                Kind::Folder => "folder",
                Kind::Entry => "entry",
            }
        ));
    }
    let Some(entry) = node.get(0) else {
        return Err(format!("`{name}` needs a value"));
    };
    let value = if let Some(text) = entry.as_string() {
        text.to_owned()
    } else if let Some(flag) = entry.as_bool() {
        flag.to_string()
    } else {
        return Err(format!("`{name}` needs a value"));
    };
    if value == "inherit" {
        return Ok(Prop::Inherit);
    }
    match parse(&value) {
        Some(value) => Ok(Prop::Set(value)),
        None => Err(format!("`{name}` does not accept `{value}`")),
    }
}

fn parse_view(value: &str) -> Option<ViewMode> {
    match value {
        "list" => Some(ViewMode::List),
        "grid" => Some(ViewMode::Grid),
        _ => None,
    }
}

fn parse_direction(value: &str) -> Option<SortDir> {
    match value {
        "ascending" => Some(SortDir::Ascending),
        "descending" => Some(SortDir::Descending),
        _ => None,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_matcher(node: &KdlNode, kind: Kind) -> Result<Matcher, String> {
    let mut matcher = Matcher::default();
    if let Some(path) = node.get(0).and_then(|entry| entry.as_string()) {
        matcher.path = Some(path.to_owned());
    }
    for entry in node.entries() {
        let Some(key) = entry.name().map(|name| name.value()) else {
            continue;
        };
        let value = entry
            .value()
            .as_string()
            .map(str::to_owned)
            .or_else(|| entry.value().as_integer().map(|number| number.to_string()))
            .ok_or_else(|| format!("`{key}` needs a string or a number"))?;
        match key {
            "path" => matcher.path = Some(value),
            "name" => matcher.name = Some(value),
            "kind" => {
                matcher.kind = Some(match value.as_str() {
                    "file" => EntryKind::File,
                    "folder" => EntryKind::Folder,
                    other => return Err(format!("`kind` does not accept `{other}`")),
                })
            }
            "extension" => matcher.extension = Some(value),
            "min-items" => matcher.min_items = Some(parse_count(key, &value)?),
            "max-items" => matcher.max_items = Some(parse_count(key, &value)?),
            other => return Err(format!("unknown matcher `{other}`")),
        }
    }
    if kind == Kind::Folder && (matcher.name.is_some() || matcher.kind.is_some() || matcher.extension.is_some()) {
        return Err(
            "folder rules match path, min-items, and max-items; name, kind, and extension are \
             for entry rules"
                .to_owned(),
        );
    }
    Ok(matcher)
}

fn parse_count(key: &str, value: &str) -> Result<usize, String> {
    value
        .parse()
        .map_err(|_| format!("`{key}` needs a whole number, not `{value}`"))
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
        b'?' => {
            !text.is_empty()
                && !is_separator(text[0])
                && glob_bytes(&pattern[1..], &text[1..])
        }
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
