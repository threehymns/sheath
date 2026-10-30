//! The Rules tab's editing: turning what is on screen into a rule, deleting one, and moving one
//! in the file's order. Every change goes through the config store, so the rest of the file
//! keeps its comments and layout.

use crate::app::{App, SettingsTab, ViewMode};
use crate::config;
use crate::rules::{EntryKind, EntryRef, Kind, Matcher, Prop, Rule, SortKey};
use kdl::{KdlDocument, KdlNode};
use rune::TextEdit;
use rune_kit::SortDir;
use std::collections::BTreeMap;
use std::path::Path;

/// The height of one rule's row. The drag handler turns a pointer's travel into rows with it.
pub const RULE_ROW_HEIGHT: f32 = 56.0;

/// One property the editor can change, in the button that cycles it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleProp {
    View,
    Sort,
    Direction,
    ShowHidden,
    Hidden,
}

/// The rule open in the tab's editor: the draft every control edits, and one text field per
/// string the draft holds.
pub struct RuleEdit {
    /// The rule's position in the file's order.
    pub index: usize,
    /// The rule as the controls have left it; written on every change.
    pub draft: Rule,
    /// The text fields, keyed by the part of the rule they hold (`name`, `m0.path`, …).
    pub fields: BTreeMap<String, TextEdit>,
}

impl RuleEdit {
    fn new(index: usize, rule: &Rule) -> RuleEdit {
        let mut edit = RuleEdit {
            index,
            draft: rule.clone(),
            fields: BTreeMap::new(),
        };
        edit.reseed();
        edit
    }

    /// Builds the text fields for the draft as it stands now. Structural changes (a new matcher,
    /// another rule kind) shift the keys, so the fields are rebuilt from the draft.
    fn reseed(&mut self) {
        self.fields.clear();
        let name = self.draft.name.clone().unwrap_or_default();
        self.fields.insert("name".to_owned(), TextEdit::new(name));
        for (list, matchers) in [("m", &self.draft.matches), ("x", &self.draft.excludes)] {
            for (index, matcher) in matchers.iter().enumerate() {
                seed_matcher(&mut self.fields, list, index, matcher, self.draft.kind);
            }
        }
    }
}

fn seed_matcher(
    fields: &mut BTreeMap<String, TextEdit>,
    list: &str,
    index: usize,
    matcher: &Matcher,
    kind: Kind,
) {
    let mut put = |key: &str, value: String| {
        fields.insert(format!("{list}{index}.{key}"), TextEdit::new(value));
    };
    put("path", matcher.path.clone().unwrap_or_default());
    match kind {
        Kind::Folder => {
            put(
                "min-items",
                matcher.min_items.map(|n| n.to_string()).unwrap_or_default(),
            );
            put(
                "max-items",
                matcher.max_items.map(|n| n.to_string()).unwrap_or_default(),
            );
        }
        Kind::Entry => {
            put("name", matcher.name.clone().unwrap_or_default());
            put("extension", matcher.extension.clone().unwrap_or_default());
        }
    }
}

/// The next value of a property's cycle: unset, each value the file accepts, then `inherit`.
fn cycle<T: Copy + PartialEq>(prop: Prop<T>, steps: &[Prop<T>]) -> Prop<T> {
    let at = steps.iter().position(|step| *step == prop).unwrap_or(0);
    steps[(at + 1) % steps.len()]
}

impl App {
    /// A path with the home folder written as `~`, so a rule travels between machines.
    pub fn short_path(path: &Path) -> String {
        let text = path.to_string_lossy();
        let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
            return text.into_owned();
        };
        let home = home.to_string_lossy();
        let home = home.trim_end_matches(['/', '\\']);
        match text.strip_prefix(home) {
            Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => format!("~{rest}"),
            _ => text.into_owned(),
        }
    }

    /// "Save this view as a rule": pins the current folder's view, sort, direction, and hidden
    /// policy in a new folder rule, appended so it applies immediately.
    pub fn save_view_rule(&mut self) {
        let pattern = App::short_path(&self.cwd);
        let name = self
            .cwd
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "folder".to_owned());
        let view = if self.view.is_grid() { "grid" } else { "list" };
        let direction = match self.sort.1 {
            SortDir::Ascending => "ascending",
            SortDir::Descending => "descending",
        };
        let hidden = if self.show_hidden { "true" } else { "false" };
        let text = format!(
            "folder-rule {} {{\n    match path={}\n    view {view}\n    sort {}\n    direction {direction}\n    show-hidden #{hidden}\n}}\n",
            crate::rules::quote(&name),
            crate::rules::quote(&pattern),
            SortKey::from_column(&self.sort.0).unwrap_or(SortKey::Name).as_str(),
        );
        self.append_rule(&text);
    }

    /// "Always show this entry" or "Always hide this entry", for the selected row.
    pub fn pin_entry(&mut self, hidden: bool) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(entry) = self.entries.iter().find(|entry| entry.id == id) else {
            return;
        };
        let pattern = App::short_path(&entry.path);
        let name = entry.name.clone();
        let hidden = if hidden { "true" } else { "false" };
        let text = format!(
            "entry-rule {} {{\n    match path={}\n    hidden #{hidden}\n}}\n",
            crate::rules::quote(&name),
            crate::rules::quote(&pattern),
        );
        self.append_rule(&text);
    }

    /// Opens the editor on `index`, or closes it when it is already open on that rule.
    pub fn toggle_rule_edit(&mut self, index: usize) {
        if self
            .rule_edit
            .as_ref()
            .is_some_and(|edit| edit.index == index)
        {
            self.rule_edit = None;
            return;
        }
        if let Some(rule) = self.rules.rules.get(index) {
            self.rule_edit = Some(RuleEdit::new(index, rule));
        }
    }

    pub fn close_rule_edit(&mut self) {
        self.rule_edit = None;
    }

    /// The editor's text fields, for `TextInput::on_edit`.
    pub fn rule_fields_mut(&mut self) -> &mut BTreeMap<String, TextEdit> {
        &mut self
            .rule_edit
            .as_mut()
            .expect("the editor's fields belong to an open rule")
            .fields
    }

    /// Reads every text field back into the draft and writes it.
    pub fn apply_rule_fields(&mut self) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let RuleEdit { draft, fields, .. } = edit;
        let name = fields
            .get("name")
            .map(|field| field.text.trim().to_owned())
            .unwrap_or_default();
        draft.name = (!name.is_empty()).then_some(name);
        let kind = draft.kind;
        for (list, matchers) in [("m", &mut draft.matches), ("x", &mut draft.excludes)] {
            for (index, matcher) in matchers.iter_mut().enumerate() {
                let read = |key: &str| {
                    fields
                        .get(&format!("{list}{index}.{key}"))
                        .map(|field| field.text.trim().to_owned())
                };
                let text = |key: &str| read(key).filter(|value| !value.is_empty());
                matcher.path = text("path");
                match kind {
                    Kind::Folder => {
                        matcher.min_items = match text("min-items") {
                            Some(value) => value.parse().ok().or(matcher.min_items),
                            None => None,
                        };
                        matcher.max_items = match text("max-items") {
                            Some(value) => value.parse().ok().or(matcher.max_items),
                            None => None,
                        };
                    }
                    Kind::Entry => {
                        matcher.name = text("name");
                        matcher.extension = text("extension");
                    }
                }
            }
        }
        self.write_rule_edit();
    }

    /// Appends an empty `match` or `exclude` directive, which the fields then fill in.
    pub fn add_matcher(&mut self, exclude: bool) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let matchers = if exclude {
            &mut edit.draft.excludes
        } else {
            &mut edit.draft.matches
        };
        matchers.push(Matcher::default());
        edit.reseed();
        self.write_rule_edit();
    }

    pub fn remove_matcher(&mut self, exclude: bool, index: usize) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let matchers = if exclude {
            &mut edit.draft.excludes
        } else {
            &mut edit.draft.matches
        };
        if index < matchers.len() {
            matchers.remove(index);
        }
        edit.reseed();
        self.write_rule_edit();
    }

    /// Turns the rule into the other kind, taking the parts that kind cannot hold with it.
    pub fn cycle_rule_kind(&mut self) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let kind = match edit.draft.kind {
            Kind::Folder => Kind::Entry,
            Kind::Entry => Kind::Folder,
        };
        edit.draft.kind = kind;
        for matcher in edit
            .draft
            .matches
            .iter_mut()
            .chain(edit.draft.excludes.iter_mut())
        {
            match kind {
                Kind::Folder => {
                    matcher.name = None;
                    matcher.kind = None;
                    matcher.extension = None;
                }
                Kind::Entry => {
                    matcher.min_items = None;
                    matcher.max_items = None;
                }
            }
        }
        match kind {
            Kind::Folder => edit.draft.props.hidden = Prop::Unset,
            Kind::Entry => {
                edit.draft.props.view = Prop::Unset;
                edit.draft.props.sort = Prop::Unset;
                edit.draft.props.direction = Prop::Unset;
                edit.draft.props.show_hidden = Prop::Unset;
            }
        }
        edit.reseed();
        self.write_rule_edit();
    }

    /// Advances one property through the values the file accepts.
    pub fn cycle_rule_prop(&mut self, prop: RuleProp) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let props = &mut edit.draft.props;
        match prop {
            RuleProp::View => {
                props.view = cycle(
                    props.view,
                    &[
                        Prop::Unset,
                        Prop::Set(ViewMode::List),
                        Prop::Set(ViewMode::Grid),
                        Prop::Inherit,
                    ],
                )
            }
            RuleProp::Sort => {
                props.sort = cycle(
                    props.sort,
                    &[
                        Prop::Unset,
                        Prop::Set(SortKey::Name),
                        Prop::Set(SortKey::Size),
                        Prop::Set(SortKey::Modified),
                        Prop::Inherit,
                    ],
                )
            }
            RuleProp::Direction => {
                props.direction = cycle(
                    props.direction,
                    &[
                        Prop::Unset,
                        Prop::Set(SortDir::Ascending),
                        Prop::Set(SortDir::Descending),
                        Prop::Inherit,
                    ],
                )
            }
            RuleProp::ShowHidden => {
                props.show_hidden = cycle(
                    props.show_hidden,
                    &[
                        Prop::Unset,
                        Prop::Set(true),
                        Prop::Set(false),
                        Prop::Inherit,
                    ],
                )
            }
            RuleProp::Hidden => {
                props.hidden = cycle(
                    props.hidden,
                    &[
                        Prop::Unset,
                        Prop::Set(true),
                        Prop::Set(false),
                        Prop::Inherit,
                    ],
                )
            }
        }
        self.write_rule_edit();
    }

    /// Advances a matcher's `kind` through any, file and folder.
    pub fn cycle_matcher_kind(&mut self, exclude: bool, index: usize) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let matchers = if exclude {
            &mut edit.draft.excludes
        } else {
            &mut edit.draft.matches
        };
        let Some(matcher) = matchers.get_mut(index) else {
            return;
        };
        matcher.kind = match matcher.kind {
            None => Some(EntryKind::File),
            Some(EntryKind::File) => Some(EntryKind::Folder),
            Some(EntryKind::Folder) => None,
        };
        self.write_rule_edit();
    }

    /// Writes the draft over the rule's node, keeping that node's leading comment and the rest
    /// of the file as it was, then resolves the folder again.
    fn write_rule_edit(&mut self) {
        let Some(edit) = &self.rule_edit else {
            return;
        };
        let text = edit.draft.to_kdl();
        let index = edit.index;
        let written = self.store.edit(config::CONFIG, |doc| {
            let Some(&at) = rule_positions(doc).get(index) else {
                return;
            };
            let leading = doc.nodes()[at]
                .format()
                .map(|format| format.leading.clone())
                .unwrap_or_default();
            if let Ok(node) = format!("{leading}{text}").parse::<KdlNode>() {
                doc.nodes_mut()[at] = node;
            }
        });
        self.after_rule_change();
        if let Err(err) = written {
            self.rule_error = Some(err);
        }
    }

    /// Adds a rule node at the end of the document, so it wins over every rule above it.
    fn append_rule(&mut self, text: &str) {
        let written = match text.parse::<KdlNode>() {
            Ok(node) => self.store.edit(config::CONFIG, |doc| {
                doc.nodes_mut().push(node.clone());
            }),
            Err(err) => Err(format!("Couldn’t write the rule: {err}")),
        };
        let appended = written.is_ok();
        self.after_rule_change();
        // Set after the reload, which clears it whenever the file reads back cleanly.
        if let Err(err) = written {
            self.rule_error = Some(err);
        } else if appended && self.settings == Some(SettingsTab::Rules) {
            // The new rule is the last one, so the tab shows what the action just wrote.
            let index = self.rules.rules.len().saturating_sub(1);
            self.rule_focus = Some(index);
            self.rule_reveal = Some((index, 1));
            self.rule_query = None;
        }
    }

    /// Removes the rule at `index` in the tab's order.
    pub fn delete_rule(&mut self, index: usize) {
        // The list on screen came from the last file that parsed, so its positions mean nothing
        // against a document that has not.
        if self.rule_error.is_some() {
            return;
        }
        // The editor's indices would name the wrong rule after this.
        self.rule_edit = None;
        let _ = self.store.edit(config::CONFIG, |doc| {
            if let Some(&at) = rule_positions(doc).get(index) {
                doc.nodes_mut().remove(at);
            }
        });
        self.after_rule_change();
    }

    /// Moves the rule at `from` to `to`, rewriting the order of the rule nodes only.
    pub fn move_rule(&mut self, from: usize, to: usize) {
        if from == to || self.rule_error.is_some() {
            return;
        }
        // The editor's indices would name the wrong rule after this.
        self.rule_edit = None;
        let _ = self.store.edit(config::CONFIG, |doc| {
            let positions = rule_positions(doc);
            if from >= positions.len() || to >= positions.len() {
                return;
            }
            let mut rules: Vec<KdlNode> = positions
                .iter()
                .map(|&at| doc.nodes()[at].clone())
                .collect();
            let moved = rules.remove(from);
            rules.insert(to, moved);
            for (at, node) in positions.into_iter().zip(rules) {
                doc.nodes_mut()[at] = node;
            }
        });
        self.after_rule_change();
    }

    /// Opens the picker that lists every rule by its matcher.
    pub fn open_rule_picker(&mut self) {
        self.rule_picker = Some(0);
    }

    pub fn close_rule_picker(&mut self) {
        self.rule_picker = None;
    }

    /// Moves the picker's highlight, stopping at the ends.
    pub fn rule_picker_move(&mut self, delta: isize) {
        let count = self.rules.rules.len();
        if count == 0 {
            return;
        }
        let at = self.rule_picker.unwrap_or(0) as isize;
        self.rule_picker = Some((at + delta).clamp(0, count as isize - 1) as usize);
    }

    /// Jumps from the picker to a rule's definition in the Rules tab.
    pub fn choose_rule(&mut self, index: usize) {
        self.rule_picker = None;
        if index < self.rules.rules.len() {
            self.rule_focus = Some(index);
            self.rule_reveal = Some((index, 1));
            self.rule_query = None;
            self.settings = Some(SettingsTab::Rules);
        }
    }

    pub fn confirm_rule_picker(&mut self) {
        if let Some(index) = self.rule_picker {
            self.choose_rule(index);
        }
    }

    /// Opens the Rules tab at the first rule matching the folder on screen.
    pub fn show_folder_rules(&mut self) {
        let count = self.entries.len();
        let focus = self.rules.first_folder(&self.cwd, count);
        self.rule_focus = focus;
        self.rule_reveal = focus.map(|index| (index, 1));
        self.rule_query = focus
            .is_none()
            .then(|| "No rule matches this folder.".to_owned());
        self.settings = Some(SettingsTab::Rules);
    }

    /// Opens the Rules tab at the first rule matching the selected entry.
    pub fn show_entry_rules(&mut self) {
        let focus = self.selected.as_ref().and_then(|id| {
            let entry = self.entries.iter().find(|entry| &entry.id == id)?;
            let path = entry.path.to_string_lossy();
            self.rules.first_entry(&EntryRef {
                path: &path,
                name: &entry.name,
                is_dir: entry.is_dir,
            })
        });
        self.rule_focus = focus;
        self.rule_reveal = focus.map(|index| (index, 1));
        self.rule_query = focus
            .is_none()
            .then(|| "No rule matches the selected entry.".to_owned());
        self.settings = Some(SettingsTab::Rules);
    }

    /// Re-reads the rules and the folder after the document changed.
    fn after_rule_change(&mut self) {
        self.reload_rules();
        self.visit = Default::default();
        self.reload();
    }
}

/// The document positions of the rule nodes, in file order.
fn rule_positions(doc: &KdlDocument) -> Vec<usize> {
    doc.nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            matches!(node.name().value(), "folder-rule" | "entry-rule")
        })
        .map(|(at, _)| at)
        .collect()
}
