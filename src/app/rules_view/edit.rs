//! The rule form: the draft every control edits, the fields it holds, and the Save, Cancel, and
//! menus that act on it. Nothing here writes the file until Save is pressed.

use crate::app::App;
use crate::config;
use crate::rules::{EntryKind, Kind, Matcher, Prop, Rule};
use kdl::KdlNode;
use rune::TextEdit;
use std::collections::BTreeMap;

use super::prop::{RuleMenu, RuleProp};
use super::rule_positions;

/// The rule open in the tab's editor: the draft every control edits, and one text field per
/// string the draft holds.
pub struct RuleEdit {
    /// The rule's position in the file's order.
    pub index: usize,
    /// The rule as the controls have left it; written only when Save is pressed.
    pub draft: Rule,
    /// The text fields, keyed by the part of the rule they hold (`name`, `m0.path`, …).
    pub fields: BTreeMap<String, TextEdit>,
    /// The rule as it was when the form opened, so Cancel can be honest about losing work.
    pub original: Rule,
    /// Why Save would refuse, in the form's own words.
    pub error: Option<String>,
    /// Whether the form last drew itself with something to save. Typing lands at once, so the
    /// frame after it catches this up: Save then arrives or leaves on an animated commit
    /// instead of snapping with the keystroke.
    pub shown_dirty: bool,
    /// The menu that is open, if one is.
    pub menu: Option<RuleMenu>,
}

impl RuleEdit {
    fn new(index: usize, rule: &Rule) -> RuleEdit {
        let mut edit = RuleEdit {
            index,
            draft: rule.clone(),
            fields: BTreeMap::new(),
            original: rule.clone(),
            error: None,
            shown_dirty: false,
            menu: None,
        };
        edit.reseed();
        edit
    }

    /// Whether the form holds changes the file does not: the draft differs from the rule as it
    /// opened, or a field holds text the draft could not take (a count that is not a number),
    /// which Save would refuse. The written form is compared because `Rule` carries no equality
    /// of its own.
    pub fn dirty(&self) -> bool {
        if self.draft.to_kdl() != self.original.to_kdl() {
            return true;
        }
        let mut seeded = BTreeMap::new();
        seed(&mut seeded, &self.draft);
        seeded.iter().any(|(key, seeded)| {
            self.fields
                .get(key)
                .is_none_or(|field| field.text != seeded.text)
        })
    }

    /// Drops what the form holds and goes back to the rule as it was opened.
    pub fn discard(&mut self) {
        self.draft = self.original.clone();
        self.error = None;
        self.reseed();
    }

    /// Builds the text fields for the draft as it stands now. Structural changes (a new matcher,
    /// another rule kind) shift the keys, so the fields are rebuilt from the draft.
    fn reseed(&mut self) {
        // A structural change (another kind, a matcher added or dropped) leaves the open menu
        // naming a control that no longer exists.
        self.menu = None;
        seed(&mut self.fields, &self.draft);
    }
}

/// Builds the text fields `rule` asks for: its name, and the keys each matcher holds.
fn seed(fields: &mut BTreeMap<String, TextEdit>, rule: &Rule) {
    fields.clear();
    let name = rule.name.clone().unwrap_or_default();
    fields.insert("name".to_owned(), TextEdit::new(name));
    for (list, matchers) in [("m", &rule.matches), ("x", &rule.excludes)] {
        for (index, matcher) in matchers.iter().enumerate() {
            seed_matcher(fields, list, index, matcher, rule.kind);
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

/// What the form cannot save as it stands: a matcher that names nothing (the file reads an
/// empty `match` as matching every folder, and an empty `exclude` as vetoing the rule
/// everywhere), and a count that is not a whole number.
fn form_problem(draft: &Rule, fields: &BTreeMap<String, TextEdit>) -> Option<String> {
    let names_nothing = |matcher: &Matcher| {
        matcher.path.is_none()
            && matcher.name.is_none()
            && matcher.kind.is_none()
            && matcher.extension.is_none()
            && matcher.min_items.is_none()
            && matcher.max_items.is_none()
    };
    if draft.matches.iter().any(names_nothing) {
        return Some(
            "A match with no fields matches every folder. Fill in a path or remove the match."
                .to_owned(),
        );
    }
    if draft.excludes.iter().any(names_nothing) {
        return Some(
            "An exclude with no fields vetoes the rule in every folder. Fill in a path or remove \
             the exclude."
                .to_owned(),
        );
    }
    // A count the draft could not take lives only in the text, so the fields are checked too.
    for (list, matchers) in [("m", &draft.matches), ("x", &draft.excludes)] {
        for (index, _) in matchers.iter().enumerate() {
            for (key, label) in [("min-items", "min items"), ("max-items", "max items")] {
                let Some(field) = fields.get(&format!("{list}{index}.{key}")) else {
                    continue;
                };
                let text = field.text.trim();
                if text.is_empty() || text.parse::<usize>().is_ok() {
                    continue;
                }
                return Some(format!("`{label}` needs a whole number, not `{text}`."));
            }
        }
    }
    for matchers in [&draft.matches, &draft.excludes] {
        for matcher in matchers {
            if let (Some(min), Some(max)) = (matcher.min_items, matcher.max_items)
                && min > max
            {
                return Some(
                    "`min items` is larger than `max items`, so the rule can never match."
                        .to_owned(),
                );
            }
        }
    }
    None
}

impl App {
    /// Opens the editor on `index`, in place of whatever form was open.
    pub fn open_rule_edit(&mut self, index: usize) {
        if let Some(rule) = self.rules.rules.get(index) {
            self.rule_edit = Some(RuleEdit::new(index, rule));
        }
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
        self.open_rule_edit(index);
    }

    pub fn close_rule_edit(&mut self) {
        self.rule_edit = None;
    }

    /// Writes the form's draft over the rule and closes the form. A draft the file cannot hold
    /// is refused, and the form says why.
    pub fn save_rule_edit(&mut self) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        edit.error = form_problem(&edit.draft, &edit.fields);
        if edit.error.is_some() {
            return;
        }
        self.write_rule_edit();
        self.rule_edit = None;
        // The row is left unfocused: the focused fill answers a command that singles a rule
        // out, and saving one the user was already looking at is not that.
    }

    /// Closes the form, dropping whatever it holds.
    pub fn discard_rule_edit(&mut self) {
        if let Some(edit) = self.rule_edit.as_mut() {
            edit.discard();
        }
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

    /// Reads every text field back into the draft and notes what would stop it being saved.
    /// Nothing is written here: the file changes when the form is saved.
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
        edit.error = form_problem(draft, fields);
        edit.shown_dirty = edit.dirty();
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
        self.apply_rule_fields();
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
        self.apply_rule_fields();
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
        self.apply_rule_fields();
    }

    /// Opens `menu`, or closes it when it is the one already open.
    pub fn toggle_rule_menu(&mut self, menu: RuleMenu) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let empty = menu == RuleMenu::AddProperty
            && RuleProp::all(edit.draft.kind)
                .iter()
                .all(|prop| prop.is_set(&edit.draft.props));
        edit.menu = if empty || edit.menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
    }

    pub fn close_rule_menu(&mut self) {
        if let Some(edit) = self.rule_edit.as_mut() {
            edit.menu = None;
        }
    }

    /// Adds a property the rule does not set yet, with the value it starts at.
    pub fn add_rule_prop(&mut self, name: &str) {
        let Some(prop) = RuleProp::from_name(name) else {
            return;
        };
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        prop.set(&mut edit.draft.props, prop.initial());
        edit.menu = None;
        self.apply_rule_fields();
    }

    /// Sets one property from the value its field offered.
    pub fn set_rule_prop(&mut self, prop: RuleProp, value: &str) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        prop.set(&mut edit.draft.props, value);
        edit.menu = None;
        self.apply_rule_fields();
    }

    /// Takes one property out of the draft, so the rule leaves it to the rules above it.
    pub fn clear_rule_prop(&mut self, prop: RuleProp) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        let props = &mut edit.draft.props;
        match prop {
            RuleProp::View => props.view = Prop::Unset,
            RuleProp::Sort => props.sort = Prop::Unset,
            RuleProp::Direction => props.direction = Prop::Unset,
            RuleProp::ShowHidden => props.show_hidden = Prop::Unset,
            RuleProp::Hidden => props.hidden = Prop::Unset,
        }
        edit.menu = None;
        self.apply_rule_fields();
    }

    /// Sets a matcher's `kind` from the value its field offered: `any`, `file` or `folder`.
    pub fn set_matcher_kind(&mut self, exclude: bool, index: usize, value: &str) {
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
        matcher.kind = match value {
            "file" => Some(EntryKind::File),
            "folder" => Some(EntryKind::Folder),
            _ => None,
        };
        edit.menu = None;
        self.apply_rule_fields();
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
}
