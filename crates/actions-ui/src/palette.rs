use actions::{Action, Keybinds};
use rune::{Cursor, El, Icon, Key, KeyEvent, TextEdit, Tx, col, el, icon, row, text};
use rune_kit::{Kit, TextInput, glyphs, scrim};

use crate::keycaps::{KeySize, tray};

pub const ROW_HEIGHT: f32 = 32.0;
pub const ROW_GAP: f32 = 1.0;
// Rows kept between the highlight and the edge it scrolls to.
pub const CLEARANCE_ROWS: f32 = 3.0;

/// The command bar's own state: what is typed and which row is highlighted. The host stores it
/// while the bar is open and drops it when it closes.
pub struct Palette {
    pub query: TextEdit,
    pub selected: usize,
}

impl Palette {
    pub fn open() -> Palette {
        let mut query = TextEdit::new("");
        query.focused = true;
        Palette { query, selected: 0 }
    }

    pub fn actions<A: Action>(&self) -> Vec<A> {
        actions::matching(&self.query.text)
    }

    pub fn requery(&mut self, tx: &mut Tx) {
        self.selected = 0;
        tx.scroll_to_y("command-scroll", 0.0);
    }

    pub fn move_by<A: Action>(&mut self, delta: isize, tx: &mut Tx) {
        let actions = self.actions::<A>();
        if actions.is_empty() {
            self.requery(tx);
            return;
        }
        let last = actions.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
        let action = actions[self.selected];
        tx.scroll_into_view(format!("command-{}", action.id()));
    }
}

/// What the command bar needs from the app: the state it stores, the chords, and what a chosen
/// command does. The crate decides nothing about whether a command can run.
pub trait PaletteHost<A: Action>: Sized {
    fn palette(&self) -> &Palette;
    fn palette_mut(&mut self) -> &mut Palette;
    fn keybinds(&self) -> &Keybinds<A>;
    /// The action that opens the palette, so the same chord closes it.
    fn palette_action(&self) -> A;
    fn palette_enabled(&self, action: A) -> bool;
    fn palette_run(&mut self, action: A, tx: &mut Tx);
    fn palette_close(&mut self);
    /// Wraps the list in the host's scroll area. Each app keeps its own scroll bookkeeping.
    fn palette_scroll(&self, content: El) -> El;

    fn palette_edit(&mut self) -> &mut TextEdit {
        &mut self.palette_mut().query
    }

    fn palette_actions(&self) -> Vec<A> {
        self.palette().actions()
    }

    fn palette_requery(&mut self, tx: &mut Tx) {
        self.palette_mut().requery(tx);
    }

    fn palette_move(&mut self, delta: isize, tx: &mut Tx) {
        self.palette_mut().move_by::<A>(delta, tx);
    }

    fn palette_submit(&mut self, tx: &mut Tx) {
        let actions = self.palette_actions();
        let selected = self.palette().selected;
        let action = actions.get(selected).copied();
        self.palette_close();
        if let Some(action) = action {
            self.palette_run(action, tx);
        }
    }

    /// Escape and the palette chord close it; Up, Down and Return drive it. Anything else is
    /// left for the field to type.
    fn palette_key(&mut self, event: &KeyEvent, tx: &mut Tx) -> bool {
        if matches!(event.key, Key::Escape) {
            self.palette_close();
            return true;
        }
        let opens = self.palette_action();
        if self.keybinds().matches(opens, event) {
            self.palette_close();
            return true;
        }
        if event.mods.cmd || event.mods.ctrl || event.mods.alt {
            return false;
        }
        match event.key {
            Key::Down => self.palette_move(1, tx),
            Key::Up => self.palette_move(-1, tx),
            Key::Enter if !tx.key_repeat() => self.palette_submit(tx),
            _ => return false,
        }
        true
    }
}

/// The scrim and the panel, in the order they are pushed over the window.
pub fn layers<A: Action, H: PaletteHost<A> + 'static>(
    s: &H,
    k: &Kit,
    icon_of: fn(A, &Kit) -> &'static Icon,
) -> (El, El) {
    let palette = s.palette();
    let program: Vec<A> = palette.actions();
    let selected = palette.selected;
    let panel = if k.dark { k.c.bg_base } else { k.c.layer_02 };
    let field = TextInput::new(&palette.query)
        .placeholder("Type a command")
        .leading_icon(k.glyph.search)
        .width(None)
        .plain()
        .on_edit(|s: &mut H| s.palette_edit())
        .on_change(|s: &mut H, tx| s.palette_requery(tx))
        .on_submit(|s: &mut H, tx| s.palette_submit(tx))
        .on_key(|s: &mut H, event, tx| s.palette_key(event, tx))
        .build(k)
        .key("command-field");
    // Padding lives on the list, not on each row: margins on a `w_full` row would overflow it.
    let mut list = col()
        .key("command-list")
        .w_full()
        .gap(ROW_GAP)
        .px(6.0)
        .py(6.0);
    if program.is_empty() {
        list = list.child(
            row().key("command-none").px(12.0).py(14.0).child(
                text("No commands match")
                    .font(k.ty.ui)
                    .color(k.c.text_faint),
            ),
        );
    }
    for (index, action) in program.iter().copied().enumerate() {
        let enabled = s.palette_enabled(action);
        let is_selected = index == selected;
        let mut line = row()
            .key(format!("command-{}", action.id()))
            .w_full()
            .h(ROW_HEIGHT)
            .px(8.0)
            .gap(10.0)
            .center_y()
            .radius(k.radius.sm)
            .child(icon(icon_of(action, k), 14.0).color(if enabled {
                k.c.icon_muted
            } else {
                k.c.icon_faint
            }))
            .child(
                text(action.label())
                    .font(k.ty.ui)
                    .color(if enabled {
                        k.c.text_base
                    } else {
                        k.c.text_faint
                    })
                    .grow(1.0)
                    .truncate(),
            );
        let caps = s.keybinds().keycaps(action);
        if !caps.is_empty() {
            let hover = k.c.hover;
            let surface = if is_selected {
                hover.mix(panel, 1.0 - hover.a)
            } else {
                panel
            };
            line = line.child(tray(
                k,
                format!("keys-{}", action.id()),
                &caps,
                surface,
                KeySize::Small,
            ));
        }
        if is_selected {
            line = line.fill(k.c.hover);
        }
        if enabled {
            line = line
                .cursor(Cursor::Pointer)
                .on_press_tx(move |s: &mut H, tx: &mut Tx| {
                    s.palette_close();
                    s.palette_run(action, tx);
                });
        }
        list = list.child(line);
    }
    let panel = col()
        .key("command-bar")
        .w(560.0)
        .max_h(440.0)
        .radius(10.0)
        .fill(panel)
        .border(1.0, k.c.border_muted)
        .clip_rounded(10.0)
        .modal(true, |s: &mut H| s.palette_close())
        .child(row().px(6.0).py(6.0).child(field))
        .child(
            el().key("command-rule")
                .h(1.0)
                .w_full()
                .fill(k.c.border_muted),
        )
        .child(s.palette_scroll(list));
    (
        scrim("command-scrim", |s: &mut H| s.palette_close()).fill(k.alpha(k.c.black, 0.28)),
        col()
            .key("command-slot")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .pt(72.0)
            .child(row().w_full().justify_center().child(panel)),
    )
}

/// The icon drawn beside a command in the palette. Kept here so an app with no icons still gets
/// a glyph.
pub fn default_icon(_: &Kit) -> &'static Icon {
    &glyphs::KEYBOARD
}
