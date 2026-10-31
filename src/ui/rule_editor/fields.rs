//! The form's text fields: one line each, wired to the draft's own edits, and the name that
//! heads the form. Typing only changes the draft; the file is written when the form is saved.

use crate::app::App;
use rune::{El, TextEdit, TextStyle, Tx};
use rune_kit::{Kit, TextInput};
use std::collections::BTreeMap;

/// The form's title: the rule's name, set a step above the controls under it. The field's own
/// padding sits before its text, so it is pulled back by that much to line the name up with the
/// sections beneath it.
const NAME_TYPE: TextStyle = TextStyle::new(16.0, 500.0);
const NAME_PADDING: f32 = 8.0;

/// The form's head: the rule's name in the form's title type, with no fill or border around
/// it, so it reads as the form's heading until the pointer or the caret lands in it. It fills
/// the line, so a long name has the whole width to run in.
pub(super) fn rule_name_field(k: &Kit, fields: &BTreeMap<String, TextEdit>) -> El {
    rule_input(
        k,
        "name",
        TextInput::new(fields.get("name").expect("the editor seeds every field"))
            .placeholder("Rule name")
            .width(None)
            .text_style(NAME_TYPE.lh(24.0))
            .plain(),
    )
    .ml(-NAME_PADDING)
}

/// One text field of the editor, wired back to the draft. Typing only changes the draft; the
/// file is written when the form is saved.
pub(super) fn rule_field(
    k: &Kit,
    fields: &BTreeMap<String, TextEdit>,
    key: &str,
    placeholder: &str,
    width: f32,
) -> El {
    rule_input(
        k,
        key,
        TextInput::new(fields.get(key).expect("the editor seeds every field"))
            .placeholder(placeholder)
            .width(Some(width)),
    )
}

/// A field of the form, wired to the draft's own edits: the field is found by `key`, and it
/// reads and writes that entry of the draft.
fn rule_input(k: &Kit, key: &str, input: TextInput) -> El {
    let id = key.to_owned();
    let target = id.clone();
    input
        .on_edit(move |s: &mut App| {
            s.rule_fields_mut()
                .get_mut(&target)
                .expect("the editor seeds every field")
        })
        .on_change(|s: &mut App, tx: &mut Tx| {
            // A keystroke is the user's own hand, so its commit lands at once and cannot
            // animate. Save is drawn from `shown_dirty`, left as it was here: the next frame
            // catches it up, and that commit is the one that animates. Nothing else would ask
            // for that frame once the key is drawn, so this does.
            let shown = s.rule_edit.as_ref().map(|edit| edit.shown_dirty);
            s.apply_rule_fields();
            if let (Some(shown), Some(edit)) = (shown, s.rule_edit.as_mut()) {
                if edit.dirty() != shown {
                    tx.wake_after(std::time::Duration::ZERO);
                }
                edit.shown_dirty = shown;
            }
        })
        .build(k)
        .key(format!("rule-field-{id}"))
}
