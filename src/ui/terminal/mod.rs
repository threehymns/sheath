#[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
mod rows;
mod surface;

pub(crate) use surface::{screen_of, terminal_cell, terminal_drag};

use crate::app::App;
use crate::icons;
#[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
use crate::term::grid::CursorShape;
use crate::term::{ProgressKind, Terminal};
#[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
use rows::line_row;
use rune::{Cursor, Drag, El, Key, Phase, Transition, Tx, col, el, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit};

pub(super) const GRIP_HEIGHT: f32 = 7.0;

/// The screen's horizontal padding: the pointer's local points include it, a cell does not.
const PAD: f32 = 10.0;

/// The search field's type, small enough to sit inside the header's 26-point bar instead of
/// forcing the pane's chrome taller.
const SEARCH_TYPE: rune::TextStyle = rune::TextStyle::new(12.0, 500.0);

// The pane's edge is its handle: dragging it sets the height, and the shell reflows to the grid
// the new rect fits as it moves. Pushed past the minimum it collapses, and the same handle sits
// at the bottom of the frame to drag the pane back open. It is pinned rather than laid out, so
// opening or collapsing the pane never moves the rows above it.
pub(super) fn grip(s: &App, k: &'static Kit) -> El {
    let height = s.term.height();
    let open = s.term.is_open();
    let mut grip = el()
        .key("terminal-grip")
        .transition(Transition::none())
        .pin(
            Some(0.0),
            None,
            Some(0.0),
            // The line rides the pane's edge: pinned to the pane's top while it is open, and
            // to the frame's bottom edge while it is collapsed.
            Some(if open { height } else { 0.0 }),
        )
        .h(GRIP_HEIGHT)
        .pt(GRIP_HEIGHT - 1.0)
        .cursor(Cursor::ResizeRow)
        .hover(k.alpha(k.c.text_accent, 0.25))
        .on_drag(
            move |s: &mut App, drag: &mut Drag, _: &mut Tx| match drag.phase {
                Phase::Down => {
                    // A collapsed pane has no visible height: the drag up from the frame edge
                    // is measured from zero, so the height follows the pointer.
                    drag.data = if s.term.is_open() { height } else { 0.0 };
                    s.term.begin_drag();
                    s.term.set_focus(false);
                }
                Phase::Move => {
                    // `dy` is travel from the press, not the handle's own position, so one
                    // gesture can collapse the pane and bring it straight back by moving up
                    // again: the target height keeps following the pointer either way.
                    if !drag.cancelled() {
                        s.term.drag(drag.data - drag.dy, s.cwd.clone());
                    }
                }
                Phase::Up => {
                    if drag.cancelled() {
                        return;
                    }
                    if s.term.is_open() {
                        s.term.set_height(drag.data - drag.dy);
                        s.remember_terminal_height();
                    } else if !s.term.did_open() && drag.dy <= 0.0 {
                        // Only a press that never showed the pane is the click that opens it;
                        // a drag that opened and collapsed it again stays collapsed.
                        s.term.expand(s.cwd.clone(), s.term.height());
                        s.remember_terminal_height();
                    }
                }
            },
        );
    // The handle paints nothing but the seam line along its bottom edge, so its whole height
    // reads as the edge between the panes instead of being half swallowed by the header below
    // it. A collapsed pane has nothing below the handle, so the line goes with it.
    if open {
        grip = grip.child(
            super::divider(k)
                .key("terminal-divider")
                .transition(Transition::none()),
        );
    }
    grip
}

// Everything the canvas is drawn from cuts in and out: Rune fades every element on mount, so
// each part has to say so, or the pane and its rows arrive as a fade over the frame behind them.
pub(super) fn pane(s: &App, k: &'static Kit) -> El {
    col()
        .key("terminal")
        .transition(Transition::none())
        .w_full()
        .h(s.term.height())
        .min_h(0.0)
        .fill(k.c.bg_base)
        .child(header(s, k))
        .child(surface::screen(s, k).grow(1.0).min_h(0.0))
}

fn header(s: &App, k: &'static Kit) -> El {
    // A program that sets no title leaves the header on the shell's own directory when it
    // reports one (OSC 7), so `cd` in the pane still moves the header even without a prompt
    // that renames the window.
    let title = if !s.term.frame.title.trim().is_empty() {
        s.term.frame.title.clone()
    } else if let Some(dir) = s.term.pwd() {
        dir.display().to_string()
    } else {
        s.cwd.display().to_string()
    };
    let mut bar = row()
        .key("terminal-header")
        .w_full()
        .h(26.0)
        .px(8.0)
        .gap(6.0)
        .center_y()
        .fill(k.c.bg_deep)
        .transition(Transition::none())
        .child(
            text(title)
                .key("terminal-title")
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .truncate()
                .transition(Transition::none()),
        );
    // The find section sits between the title and the pane's own buttons, so it reads as part
    // of the header instead of trailing the close button.
    if s.term.searching() {
        bar = bar
            .child(search_field(s, k))
            // A vertical rule separates finding from the pane's own controls, so the two read
            // as different groups instead of one run of buttons.
            .child(
                el().key("terminal-search-rule")
                    .w(1.0)
                    .h(16.0)
                    .fill(k.c.border_muted)
                    .transition(Transition::none()),
            );
    }
    bar = bar
        .child(
            IconButton::new(&icons::CHEVRON_DOWN)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label("Collapse")
                .help("Collapse the pane; its shell keeps running")
                .on_press(|s: &mut App| s.term.collapse())
                .build(k)
                .key("terminal-collapse")
                .transition(Transition::none()),
        )
        .child(
            IconButton::new(&icons::X)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label("Close")
                .help("Close the pane and end its shell")
                .on_press(|s: &mut App| s.term.close())
                .build(k)
                .key("terminal-close")
                .transition(Transition::none()),
        );
    if let Some(report) = progress(&s.term, k) {
        bar = bar.child(report);
    }
    if let Some(error) = &s.term.error {
        bar = bar.child(
            text(error.clone())
                .key("terminal-error")
                .font(k.ty.caption())
                .color(k.c.danger.fg)
                .max_lines(2)
                .transition(Transition::none()),
        );
    }
    col()
        .key("terminal-bar")
        .transition(Transition::none())
        .w_full()
        .child(bar)
        .child(
            el().w_full()
                .h(1.0)
                .fill(k.c.border_muted)
                .transition(Transition::none()),
        )
}

/// The search field, its match count and the two buttons that walk the matches: the same three
/// pieces Ghostty's own search bar shows.
fn search_field(s: &App, k: &'static Kit) -> El {
    // A field nobody has typed in has nothing to report: "No matches" there would read as the
    // scrollback having no matches rather than as a search that has not started.
    let label = if s.term.search_text().text.trim().is_empty() {
        None
    } else {
        let (total, selected) = s.term.search_state().unwrap_or((0, None));
        Some(match (total, selected) {
            (0, _) => "No matches".to_owned(),
            (total, Some(index)) => format!("{}/{total}", index + 1),
            (total, None) => format!("{total} matches"),
        })
    };
    let mut bar = row().key("terminal-search").gap(4.0).center_y().child(
        rune_kit::TextInput::new(s.term.search_text())
            .placeholder("Search the scrollback")
            .width(Some(180.0))
            .text_style(SEARCH_TYPE.lh(18.0))
            .plain()
            .on_edit(|s: &mut App| s.term.search_text_mut())
            .on_change(|s: &mut App, _: &mut Tx| {
                s.term.apply_search();
            })
            // The focused field sees its keys before the app does, so Escape and Return are
            // handled here: Escape closes the find section, Return walks the matches.
            .on_key(
                |s: &mut App, event: &rune::KeyEvent, _: &mut Tx| match event.key {
                    Key::Escape => {
                        s.term.close_search();
                        true
                    }
                    Key::Enter => {
                        if event.mods.shift {
                            s.term.search_prev();
                        } else {
                            s.term.search_next();
                        }
                        true
                    }
                    _ => false,
                },
            )
            .build(k)
            .key("terminal-search-input"),
    );
    if let Some(label) = label {
        bar = bar.child(
            text(label)
                .key("terminal-search-count")
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .transition(Transition::none()),
        );
    }
    bar.child(
        IconButton::new(&icons::ARROW_UP)
            .variant(IconButtonVariant::GhostMuted)
            .control_size(ControlSize::Small)
            .label("Previous match")
            .help("Previous match")
            .on_press(|s: &mut App| {
                s.term.search_prev();
            })
            .build(k)
            .key("terminal-search-prev")
            .transition(Transition::none()),
    )
    .child(
        IconButton::new(&icons::ARROW_DOWN)
            .variant(IconButtonVariant::GhostMuted)
            .control_size(ControlSize::Small)
            .label("Next match")
            .help("Next match")
            .on_press(|s: &mut App| {
                s.term.search_next();
            })
            .build(k)
            .key("terminal-search-next")
            .transition(Transition::none()),
    )
    .child(
        IconButton::new(&icons::X)
            .variant(IconButtonVariant::GhostMuted)
            .control_size(ControlSize::Small)
            .label("Close search")
            .help("Close the search")
            .on_press(|s: &mut App| s.term.close_search())
            .build(k)
            .key("terminal-search-close")
            .transition(Transition::none()),
    )
}

/// OSC 9;4: the progress a program reports for its own work, drawn in the header it shares with
/// the title. A program that reports none gets no room, so the common shell is unchanged.
fn progress(t: &Terminal, k: &'static Kit) -> Option<El> {
    let report = t.progress()?;
    let (label, color) = match report.kind {
        ProgressKind::Set => (
            report
                .percent
                .map_or_else(|| "Working…".to_owned(), |percent| format!("{percent}%")),
            k.c.text_accent,
        ),
        ProgressKind::Indeterminate => ("Working…".to_owned(), k.c.text_accent),
        ProgressKind::Pause => ("Paused".to_owned(), k.c.text_muted),
        ProgressKind::Error => ("Failed".to_owned(), k.c.danger.fg),
    };
    let mut bar = row()
        .key("terminal-progress")
        .h(14.0)
        .gap(6.0)
        .center_y()
        .shrink()
        .transition(Transition::none())
        .child(
            text(label)
                .key("terminal-progress-label")
                .font(k.ty.caption())
                .color(color)
                .transition(Transition::none()),
        );
    if let Some(percent) = report.percent {
        // A determinate report gets a track as well: the number alone reads as a label, and
        // the bar is what makes it read as how much is left.
        let track = 48.0;
        let width = (track * f32::from(percent) / 100.0).max(1.0);
        bar = bar.child(
            el().key("terminal-progress-track")
                .w(track)
                .h(3.0)
                .clip_rounded(1.5)
                .fill(k.alpha(k.c.text_muted, 0.25))
                .transition(Transition::none())
                .child(
                    el().key("terminal-progress-fill")
                        .w(width)
                        .h(3.0)
                        .clip_rounded(1.5)
                        .fill(color)
                        .transition(Transition::none()),
                ),
        );
    }
    Some(bar)
}

impl App {
    /// The terminal's own chords, checked before the shell sees the key: copy the selection and
    /// paste the clipboard, which a bare Ctrl/Cmd-C and -V can't mean while a shell is typing.
    pub fn terminal_chord(&mut self, event: &rune::KeyEvent, tx: &mut Tx) -> bool {
        let m = event.mods;
        let shift = m.shift;
        if shift && event.is_command('c') {
            self.terminal_copy(tx);
            return true;
        }
        if shift && event.is_command('a') {
            self.term.select_all();
            return true;
        }
        // Cmd/Ctrl+Shift+F finds in the scrollback, as Ghostty's own search does.
        if shift && event.is_command('f') {
            self.term.open_search();
            return true;
        }
        if shift && event.is_command('v') {
            self.terminal_paste();
            return true;
        }
        false
    }
}
