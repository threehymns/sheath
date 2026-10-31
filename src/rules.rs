//! Folder and entry rules: the nodes in `config.kdl` and the presentation a folder resolves to. Everything here is pure: it takes a parsed document and a path, and returns
//! what the browser should show.

use crate::app::Layout;
use kdl::{KdlDocument, KdlNode};
use rune_kit::SortDir;
use std::path::Path;

mod pattern;

pub use pattern::{glob, path_match, quote};

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

    /// The key a column's string id names; the browser keeps the id, and this is the one place
    /// that turns it back.
    pub fn from_column(value: &str) -> Option<SortKey> {
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
            && self
                .name
                .as_ref()
                .is_none_or(|pattern| glob(pattern, entry.name))
            && self
                .extension
                .as_ref()
                .is_none_or(|pattern| glob(pattern, entry.extension()))
            && self
                .kind
                .is_none_or(|kind| (kind == EntryKind::Folder) == entry.is_dir)
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
    pub layout: Prop<Layout>,
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
        let matched =
            self.matches.is_empty() || self.matches.iter().any(|m| m.matches_folder(path, count));
        matched && !self.excludes.iter().any(|m| m.matches_folder(path, count))
    }

    fn applies_to_entry(&self, entry: &EntryRef<'_>) -> bool {
        if self.kind != Kind::Entry {
            return false;
        }
        let matched =
            self.matches.is_empty() || self.matches.iter().any(|m| m.matches_entry(entry));
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

    /// The rule's matchers and properties, as the tab and the picker print them.
    pub fn details(&self) -> String {
        let mut parts = Vec::new();
        for matcher in &self.matches {
            parts.push(format!("match {}", matcher.summary()));
        }
        for matcher in &self.excludes {
            parts.push(format!("exclude {}", matcher.summary()));
        }
        match self.props.layout {
            Prop::Unset => {}
            Prop::Inherit => parts.push("layout inherit".to_owned()),
            Prop::Set(layout) => parts.push(format!(
                "layout {}",
                if layout.is_grid() { "grid" } else { "list" }
            )),
        }
        match self.props.sort {
            Prop::Unset => {}
            Prop::Inherit => parts.push("sort inherit".to_owned()),
            Prop::Set(sort) => parts.push(format!("sort {}", sort.as_str())),
        }
        match self.props.direction {
            Prop::Unset => {}
            Prop::Inherit => parts.push("direction inherit".to_owned()),
            Prop::Set(SortDir::Ascending) => parts.push("direction ascending".to_owned()),
            Prop::Set(SortDir::Descending) => parts.push("direction descending".to_owned()),
        }
        for (name, prop) in [
            ("show-hidden", self.props.show_hidden),
            ("hidden", self.props.hidden),
        ] {
            match prop {
                Prop::Unset => {}
                Prop::Inherit => parts.push(format!("{name} inherit")),
                Prop::Set(true) => parts.push(format!("{name} #true")),
                Prop::Set(false) => parts.push(format!("{name} #false")),
            }
        }
        if parts.is_empty() {
            "everything".to_owned()
        } else {
            parts.join("  ")
        }
    }

    /// The rule as the `folder-rule` or `entry-rule` node it lives as, in the shape the parser
    /// reads back. The editor writes this over the old node, so nothing else in the file moves.
    pub fn to_kdl(&self) -> String {
        let head = match self.kind {
            Kind::Folder => "folder-rule",
            Kind::Entry => "entry-rule",
        };
        let mut text = match &self.name {
            Some(name) => format!("{head} {} {{\n", quote(name)),
            None => format!("{head} {{\n"),
        };
        for matcher in &self.matches {
            text.push_str(&directive("match", matcher));
        }
        for matcher in &self.excludes {
            text.push_str(&directive("exclude", matcher));
        }
        match self.props.layout {
            Prop::Unset => {}
            Prop::Inherit => text.push_str("    layout inherit\n"),
            Prop::Set(layout) => text.push_str(&format!(
                "    layout {}\n",
                if layout.is_grid() { "grid" } else { "list" }
            )),
        }
        match self.props.sort {
            Prop::Unset => {}
            Prop::Inherit => text.push_str("    sort inherit\n"),
            Prop::Set(sort) => text.push_str(&format!("    sort {}\n", sort.as_str())),
        }
        match self.props.direction {
            Prop::Unset => {}
            Prop::Inherit => text.push_str("    direction inherit\n"),
            Prop::Set(SortDir::Ascending) => text.push_str("    direction ascending\n"),
            Prop::Set(SortDir::Descending) => text.push_str("    direction descending\n"),
        }
        for (name, prop) in [
            ("show-hidden", self.props.show_hidden),
            ("hidden", self.props.hidden),
        ] {
            match prop {
                Prop::Unset => {}
                Prop::Inherit => text.push_str(&format!("    {name} inherit\n")),
                Prop::Set(on) => {
                    text.push_str(&format!("    {name} #{on}\n"));
                }
            }
        }
        text.push_str("}\n");
        text
    }
}

/// One `match` or `exclude` line, with only the keys the directive names.
fn directive(name: &str, matcher: &Matcher) -> String {
    let mut parts = vec![name.to_owned()];
    if let Some(path) = &matcher.path {
        parts.push(format!("path={}", quote(path)));
    }
    if let Some(value) = &matcher.name {
        parts.push(format!("name={}", quote(value)));
    }
    if let Some(kind) = matcher.kind {
        parts.push(format!(
            "kind=\"{}\"",
            match kind {
                EntryKind::File => "file",
                EntryKind::Folder => "folder",
            }
        ));
    }
    if let Some(extension) = &matcher.extension {
        parts.push(format!("extension={}", quote(extension)));
    }
    if let Some(min) = matcher.min_items {
        parts.push(format!("min-items={min}"));
    }
    if let Some(max) = matcher.max_items {
        parts.push(format!("max-items={max}"));
    }
    format!("    {}\n", parts.join(" "))
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
    pub layout: Option<Layout>,
    pub sort: Option<SortKey>,
    pub direction: Option<SortDir>,
    pub show_hidden: Option<bool>,
}

impl Resolved {
    /// Fills what this resolution leaves unset from `fallback`.
    pub fn or(self, fallback: Resolved) -> Resolved {
        Resolved {
            layout: self.layout.or(fallback.layout),
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
            merge(&mut resolved.layout, rule.props.layout);
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
    let name = node
        .get(0)
        .and_then(|entry| entry.as_string())
        .map(str::to_owned);
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
            "layout" => {
                rule.props.layout = parse_prop(child, "layout", Kind::Folder, kind, parse_layout)?
            }
            "sort" => {
                rule.props.sort =
                    parse_prop(child, "sort", Kind::Folder, kind, SortKey::from_column)?
            }
            "direction" => {
                rule.props.direction =
                    parse_prop(child, "direction", Kind::Folder, kind, parse_direction)?
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
                    "unknown property `{other}`; folder rules and entry rules support layout, \
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

fn parse_layout(value: &str) -> Option<Layout> {
    match value {
        "list" => Some(Layout::List),
        "grid" => Some(Layout::Grid),
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
    if kind == Kind::Folder
        && (matcher.name.is_some() || matcher.kind.is_some() || matcher.extension.is_some())
    {
        return Err(
            "folder rules match path, min-items, and max-items; name, kind, and extension are \
             for entry rules"
                .to_owned(),
        );
    }
    if kind == Kind::Entry && (matcher.min_items.is_some() || matcher.max_items.is_some()) {
        return Err(
            "entry rules match path, name, kind, and extension; min-items and max-items are for \
             folder rules"
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
