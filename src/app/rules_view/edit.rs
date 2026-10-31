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
    pub fn open_rule_edit(&mut self, index: usize) {
        if let Some(rule) = self.rules.rules.get(index) {
            self.rule_edit = Some(RuleEdit::opened(index, rule));
        }
    }

    pub fn open_new_rule(&mut self, kind: Kind) {
        let index = self.rules.rules.len();
        let rule = self.new_rule(kind);
        self.rule_edit = Some(RuleEdit::added(index, rule));
        self.rule_reveal = Some((format!("rule-editor-{index}"), 1));
    }

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
    }

    pub fn discard_rule_edit(&mut self) {
        if let Some(edit) = self.rule_edit.as_mut() {
            edit.discard();
        }
        self.rule_edit = None;
    }

    pub fn rule_fields_mut(&mut self) -> &mut BTreeMap<String, TextEdit> {
        &mut self
            .rule_edit
            .as_mut()
            .expect("the editor's fields belong to an open rule")
            .fields
    }

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
        edit.error = None;
        edit.shown_dirty = edit.dirty();
    }

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

    pub fn set_rule_prop(&mut self, prop: RuleProp, value: &str) {
        let Some(edit) = self.rule_edit.as_mut() else {
            return;
        };
        prop.set(&mut edit.draft.props, value);
        edit.menu = None;
        self.apply_rule_fields();
    }

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
