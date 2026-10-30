//! The form's draft: the rule every control edits, the text fields that hold its strings, and
//! what the file cannot accept as it stands. Nothing here touches the app or a file.

use crate::rules::{Kind, Matcher, Rule};
use rune::TextEdit;
use std::collections::BTreeMap;

use super::prop::RuleMenu;

/// The rule open in the tab's editor: the draft every control edits, and one text field per
/// string the draft holds.
pub struct RuleEdit {
    /// The rule's position in the file's order; for a rule the file does not hold yet, the
    /// place Save appends it at.
    pub index: usize,
    /// Whether the form is a rule the file does not hold yet, which Save appends instead of
    /// writing over whatever sits at `index`.
    pub new: bool,
    /// The rule as the controls have left it; written only when Save is pressed.
    pub draft: Rule,
    /// The text fields, keyed by the part of the rule they hold (`name`, `m0.path`, …).
    pub fields: BTreeMap<String, TextEdit>,
    /// The rule as it was when the form opened, so Cancel can be honest about losing work.
    pub original: Rule,
    /// Why the last Save was refused, in the form's own words; the next edit takes it away.
    pub error: Option<String>,
    /// Whether the form last drew itself with something to save. Typing lands at once, so the
    /// frame after it catches this up: Save then arrives or leaves on an animated commit
    /// instead of snapping with the keystroke.
    pub shown_dirty: bool,
    /// The menu that is open, if one is.
    pub menu: Option<RuleMenu>,
}

impl RuleEdit {
    /// A form on a rule the file holds.
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

    /// A form on a rule the file does not hold yet. Save appends it, so Add Rule writes a
    /// whole rule rather than a blank node that would match everything and set nothing.
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

    /// Builds the text fields for the draft as it stands now. Structural changes (a matcher
    /// added or dropped) shift the keys, so the fields are rebuilt from the draft.
    pub(super) fn reseed(&mut self) {
        // A matcher added or dropped leaves the open menu naming a control that no longer
        // exists.
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
pub(super) fn form_problem(draft: &Rule, fields: &BTreeMap<String, TextEdit>) -> Option<String> {
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
