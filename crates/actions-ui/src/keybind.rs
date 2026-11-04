use actions::{Action, Keybinds, Recorder};
use rune::{El, Icon, PhysicalInput, col, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit, shortcut_error_message};

use crate::keycaps::{KeySize, tray};
use crate::settings::{settings_row, settings_section};

/// What the keybind editor needs from the app. The host owns the bindings, their recorders and
/// the file they are saved in; the crate only draws them.
pub trait KeybindHost<A: Action>: Sized {
    /// The action's recorder, or `None` to keep that action out of the editor.
    fn recorder(&self, action: A) -> Option<&Recorder>;
    /// Puts the action's field into recording mode.
    fn begin_recording(&mut self, action: A);
    fn keybinds(&self) -> &Keybinds<A>;
    fn record_key(&mut self, action: A, event: &PhysicalInput) -> bool;
    fn reset_binding(&mut self, action: A);
    /// Where the bindings are saved, for the line under the list.
    fn keybinds_saved_in(&self) -> Option<String>;
    fn keybinds_error(&self) -> Option<String>;
}

/// The Shortcuts card: one row per action, each with a recorder field.
pub fn page<A: Action, H: KeybindHost<A> + 'static>(
    s: &H,
    k: &Kit,
    icon_of: fn(A, &Kit) -> &'static Icon,
    undo: &'static Icon,
) -> El {
    let mut rows = Vec::new();
    for action in A::ALL.iter().copied() {
        if s.recorder(action).is_none() {
            continue;
        }
        rows.push(settings_row(
            k,
            icon_of(action, k),
            action.label(),
            None,
            binding_control(s, k, action, recorder_field(s, k, action), undo),
        ));
    }
    let saved = match s.keybinds_saved_in() {
        Some(path) => format!(
            "Click a shortcut and press the keys to use instead. Backspace clears it, Escape \
             cancels while recording. Saved in {path}."
        ),
        None => "Click a shortcut and press the keys to use instead. Backspace clears it, Escape \
                 cancels while recording."
            .to_owned(),
    };
    let mut page = col()
        .key("settings-keybinds")
        .w_full()
        .gap(24.0)
        .child(settings_section(k, "settings-shortcuts", "Shortcuts", rows))
        .child(
            text(saved)
                .key("keybind-saved")
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .mx(crate::settings::ROW_INSET)
                .wrap(),
        );
    if let Some(error) = s.keybinds_error() {
        page = page.child(
            text(error)
                .key("keybind-error")
                .font(k.ty.small_medium)
                .color(k.c.danger.fg)
                .mx(crate::settings::ROW_INSET)
                .wrap(),
        );
    }
    page
}

fn recorder_field<A: Action, H: KeybindHost<A> + 'static>(s: &H, k: &Kit, action: A) -> El {
    let recorder = s
        .recorder(action)
        .expect("the page only draws actions it has a recorder for");
    let labels = recorder.keycaps(s.keybinds().chord(action));
    let placeholder = if recorder.is_recording() {
        "Press shortcut"
    } else {
        "Record shortcut"
    };
    let surface = if recorder.is_recording() {
        k.c.layer_01
    } else {
        k.c.bg_base
    };
    let error = recorder
        .error()
        .map(|error| shortcut_error_message(error, recorder.refused()));
    let mut field = row()
        .key(format!("record-{}-field", action.id()))
        .role("button")
        .label(if labels.is_empty() {
            placeholder.to_owned()
        } else {
            labels.join(" ")
        })
        .min_w(96.0)
        .h(28.0)
        .px(6.0)
        .center_y()
        .justify_end()
        .radius(k.radius.sm)
        .focusable(true)
        .focus_ring(2.0, 2.0, k.c.border_focus)
        .on_press(move |s: &mut H| s.begin_recording(action))
        .on_physical_input(move |s: &mut H, event, _| s.record_key(action, event))
        .child(if labels.is_empty() {
            text(placeholder)
                .font(k.ty.ui_compact)
                .color(k.c.text_faint)
        } else {
            tray(
                k,
                format!("record-{}-keys", action.id()),
                &labels,
                surface,
                KeySize::Regular,
            )
        });
    if recorder.is_recording() {
        field = field.fill(surface);
    }
    let mut root = col()
        .key(format!("record-{}", action.id()))
        .gap(4.0)
        .items_end()
        .child(field);
    if let Some(error) = error {
        root = root.child(
            text(error)
                .font(k.ty.small_medium)
                .color(k.c.danger.fg)
                .wrap(),
        );
    }
    root
}

fn binding_control<A: Action, H: KeybindHost<A> + 'static>(
    s: &H,
    k: &Kit,
    action: A,
    field: El,
    undo: &'static Icon,
) -> El {
    let mut row = row().gap(6.0).center_y();
    if s.keybinds().is_custom(action) {
        row = row.child(
            IconButton::new(undo)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label(format!("Reset {}", action.label()))
                .help("Reset to the default, taking the shortcut out of the config file")
                .on_press(move |s: &mut H| s.reset_binding(action))
                .build(k)
                .key(format!("reset-binding-{}", action.id())),
        );
    }
    row.child(field)
}
