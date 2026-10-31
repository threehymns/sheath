//! The rule form's actions: opening a rule as a draft, reading its fields back, saving or
//! discarding it, and the menus and properties that edit it. The draft itself lives in `draft`;
//! nothing here writes the file until Save is pressed.

use crate::app::App;
use crate::config;
use crate::rules::{EntryKind, Kind, Matcher, Prop, Props, Rule, SortKey};
use kdl::KdlNode;
use rune::TextEdit;
use std::collections::BTreeMap;

use super::draft::{RuleEdit, form_problem};
use super::prop::{RuleMenu, RuleProp};
use super::rule_positions;

impl App {
    /// Opens the editor on `index`, in place of whatever form was open.
    pub fn open_rule_edit(&mut self, index: usize) {
        if let Some(rule) = self.rules.rules.get(index) {
            self.rule_edit = Some(RuleEdit::opened(index, rule));
        }
    }

    /// Opens the editor on a rule the file does not hold yet: Save appends it, so Add Rule is
    /// the same gesture whether or not there is a rule to start from.
    pub fn open_new_rule(&mut self, kind: Kind) {
        let index = self.rules.rules.len();
        let rule = self.new_rule(kind);
        self.rule_edit = Some(RuleEdit::added(index, rule));
        // The form stands after its section's rows, below where the panel may be scrolled: the
        // tab scrolls to it as it does to a rule it has just written.
        self.rule_reveal = Some((format!("rule-editor-{index}"), 1));
    }

    /// What a new rule starts from: a folder rule keeps the folder on screen and the layout it
    /// is showing, and an entry rule the entry the browser has selected, so the form opens on
    /// something worth saving rather than on a rule that matches nothing in particular.
    fn new_rule(&self, kind: Kind) -> Rule {
        let mut rule = Rule {
            name: None,
            kind,
            matches: Vec::new(),
            excludes: Vec::new(),
            props: Props::default(),
        };
        match kind {
            Kind::Folder => {
                rule.matches.push(Matcher {
                    path: Some(App::short_path(&self.cwd)),
                    ..Matcher::default()
                });
                rule.props.layout = Prop::Set(self.layout);
                rule.props.sort =
                    Prop::Set(SortKey::from_column(&self.sort.0).unwrap_or(SortKey::Name));
                rule.props.direction = Prop::Set(self.sort.1);
                rule.props.show_hidden = Prop::Set(self.show_hidden);
            }
            Kind::Entry => {
                let entry = self
                    .selected
                    .as_ref()
                    .and_then(|id| self.entries.iter().find(|entry| &entry.id == id));
                if let Some(entry) = entry {
                    rule.matches.push(Matcher {
                        path: Some(App::short_path(&entry.path)),
                        ..Matcher::default()
                    });
                    rule.props.hidden = Prop::Set(false);
                }
            }
        }
        rule
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
    /// is refused, and the form says why. A write that did not reach the file keeps the form
    /// and its draft open, leaving `store.write_error` to say why.
    pub fn save_rule_edit(&mut self) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        edit.error = form_problem(&edit.draft, &edit.fields);
        if edit.error.is_some() {
            return;
        }
        if !self.write_rule_edit() {
            return;
        }
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

    /// Reads every text field back into the draft, taking down the last refusal. What would
    /// stop the draft being saved is judged when Save is pressed, not while it is typed.
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
        // Reading the fields is the user still working, so a refusal that no longer describes
        // the form goes away here rather than at the next Save: nothing is judged until the
        // form is handed in, so the sections never shift under a pointer that is typing.
        edit.error = None;
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
            RuleProp::Layout => props.layout = Prop::Unset,
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
    /// of the file as it was, then resolves the folder again. A rule the file did not hold yet
    /// is appended, so it lands after every rule and wins where the others disagree. Says
    /// whether the write reached the file.
    fn write_rule_edit(&mut self) -> bool {
        let Some(edit) = &self.rule_edit else {
            return false;
        };
        let text = edit.draft.to_kdl();
        let kind = edit.draft.kind;
        if edit.new {
            return self.append_rule(kind, &text);
        }
        let index = edit.index;
        // A write the store could not make is `store.write_error`'s to report; `rule_error` stays
        // for a file that does not parse, which is what makes the list read only.
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
        written.is_ok()
    }
}
