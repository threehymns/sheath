use crate::app::App;
use rune::{El, TextEdit, TextStyle, Tx};
use rune_kit::{Kit, TextInput};
use std::collections::BTreeMap;

const NAME_TYPE: TextStyle = TextStyle::new(16.0, 500.0);
const NAME_PADDING: f32 = 8.0;

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
            // A keystroke commits at once and cannot animate. Save is drawn from `shown_dirty`,
            // so the next frame catches it up and that commit animates.
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
