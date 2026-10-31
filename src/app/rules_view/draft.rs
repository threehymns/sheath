use crate::rules::{Kind, Matcher, Rule};
use rune::TextEdit;
use std::collections::BTreeMap;

use super::prop::RuleMenu;

pub struct RuleEdit {
    pub index: usize,
    pub new: bool,
    pub draft: Rule,
    pub fields: BTreeMap<String, TextEdit>,
    pub original: Rule,
    pub error: Option<String>,
    pub shown_dirty: bool,
    pub menu: Option<RuleMenu>,
}

impl RuleEdit {
    pub(super) fn opened(index: usize, rule: &Rule) -> RuleEdit {
        let mut edit = RuleEdit {
            index,
            new: false,
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

    pub(super) fn added(index: usize, rule: Rule) -> RuleEdit {
        let mut edit = RuleEdit {
            index,
            new: true,
            draft: rule.clone(),
            fields: BTreeMap::new(),
            original: rule,
            error: None,
            shown_dirty: false,
            menu: None,
        };
        edit.reseed();
        edit
    }

    // The written form is compared because `Rule` has no equality of its own. A field the draft
    // could not take, like a count that is not a number, also counts as a change.
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

    pub fn discard(&mut self) {
        self.draft = self.original.clone();
        self.error = None;
        self.reseed();
    }

    pub(super) fn reseed(&mut self) {
        self.menu = None;
        seed(&mut self.fields, &self.draft);
    }
}

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

pub(super) fn form_problem(draft: &Rule, fields: &BTreeMap<String, TextEdit>) -> Option<String> {
    let names_nothing = |matcher: &Matcher| {
        matcher.path.is_none()
            && matcher.name.is_none()
            && matcher.kind.is_none()
            && matcher.extension.is_none()
            && matcher.min_items.is_none()
            && matcher.max_items.is_none()
    };
    let target = match draft.kind {
        Kind::Folder => "folder",
        Kind::Entry => "entry",
    };
    if draft.matches.iter().any(names_nothing) {
        return Some(format!(
            "A match with no fields matches every {target}. Fill in a path or remove the match."
        ));
    }
    if draft.excludes.iter().any(names_nothing) {
        return Some(format!(
            "An exclude with no fields vetoes the rule in every {target}. Fill in a path or \
             remove the exclude."
        ));
    }
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
