//! What the text renderer paints: the rows a scroll moves, the overscan row at each edge, and
//! the rectangles cell backgrounds and cell-filling characters are drawn as.
//!
//! Desktop builds stream the grid as one texture and build no row elements, so these tests run
//! with `--features text-renderer`.

use rune_tty::Terminal;
#[cfg(feature = "text-renderer")]
use rune_tty::grid::{Frame, Line};

/// Where a fractional scroll puts the rows: the first row hangs lower by exactly the fraction,
/// and the overscan row above it covers the strip that would otherwise be background.
#[cfg(feature = "text-renderer")]
#[test]
fn the_overscan_row_covers_the_top_edge_at_every_offset() {
    use crate::tests::helpers::*;
    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    let mut frame = Frame {
        cols: 20,
        rows: 6,
        ..Frame::default()
    };
    for i in 0..6 {
        frame.lines.push(Line {
            text: format!("row{i}"),
            ..Line::default()
        });
    }
    rt.state.term.show_for_test(frame);
    rt.state.term.scroll_for_test(0.0);
    rt.frame(0.0);
    let flush = rt.presented("t0").expect("the first row").0[1];
    assert!(
        rt.presented("lead").is_none(),
        "no overscan at a whole-row offset"
    );

    for step in 1..10 {
        let fraction = step as f32 / 10.0;
        rt.state.term.frame.lead = Some(Line {
            text: "above".to_owned(),
            ..Line::default()
        });
        rt.state.term.frame.trail = Some(Line {
            text: "below".to_owned(),
            ..Line::default()
        });
        rt.state.term.scroll_for_test(fraction);
        rt.invalidate();
        rt.frame(1.0 / 60.0);
        let line = 16.0;
        let top = rt.presented("t0").expect("the first row").0[1];
        let lead = rt.presented("lead").expect("the overscan row").0[1];
        assert!(
            (top - (flush + fraction * line)).abs() < 0.01,
            "at {fraction} the first row is {top}, wanted {}",
            flush + fraction * line
        );
        assert!(
            (lead - (flush + (fraction - 1.0) * line)).abs() < 0.01,
            "at {fraction} the overscan row is {lead}, wanted {}",
            flush + (fraction - 1.0) * line
        );
        let trail = rt.presented("trail").expect("the row below").0[1];
        assert!(
            (trail - (flush + 6.0 * line + fraction * line)).abs() < 0.01,
            "at {fraction} the row below the viewport is {trail}, wanted {}",
            flush + 6.0 * line + fraction * line
        );
    }
}

/// A wheel settles the grid before the view reads it. The frame and the offset come from one
/// moment: if the scroll lands between the poll and the build, the new offset is drawn over the
/// previous rows and the content jumps back a row at every boundary it crosses.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_wheel_moves_the_drawn_rows_before_the_next_poll() {
    use rune::{Key, KeyEvent, Mods};

    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let theme = rune_tty::theme(true);
    let mut pane = Terminal::new();
    pane.open(dir.path().to_path_buf());
    let pump = |pane: &mut Terminal| {
        for _ in 0..40 {
            if pane.poll(Some(&wake), theme) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        false
    };
    assert!(pump(&mut pane), "the shell started");
    for c in "seq 1 60".chars() {
        pane.key(&KeyEvent {
            key: Key::Text(c.to_string()),
            mods: Mods::default(),
        });
    }
    pane.key(&KeyEvent {
        key: Key::Enter,
        mods: Mods::default(),
    });
    // Wheel back until the pane holds five rows of history, which is what the two notches below
    // need: the poll may report a frame the shell did not write, so "it drew" no longer means
    // "it has output", and the history itself is what says there is something to scroll.
    let mut scrolled = false;
    for _ in 0..200 {
        pane.scroll_to_bottom();
        pane.poll(Some(&wake), theme);
        if pane.wheel(pane.line * 5.0) && pane.scroll_back() == 5 {
            scrolled = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        scrolled,
        "the output above the viewport is history to scroll"
    );
    // Back to three, then the wheel below is the gesture that has to settle the frame.
    assert!(pane.wheel(-pane.line * 2.0));
    assert_eq!(pane.scroll_back(), 3);

    // No poll between the wheel and this read: the frame the view draws is already there.
    let top = pane.frame.lines[0].text.clone();
    assert!(pane.wheel(pane.line * 2.0));
    assert_eq!(pane.scroll_back(), 5);
    assert_ne!(
        pane.frame.lines[0].text, top,
        "the wheel moved the rows the view will draw"
    );
}

/// A cell background is a rectangle over its cells: flush with the row's edge, exactly as many
/// cells wide, and a full line tall, so neighbouring rows' bars meet without a gap.
#[cfg(feature = "text-renderer")]
#[test]
fn a_background_is_a_rectangle_over_its_cells() {
    use crate::tests::helpers::*;
    use rune_tty::grid::{Pen, Run};

    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    let mut line = Line {
        text: "hello".to_owned(),
        ..Line::default()
    };
    line.runs.push(Run {
        start: 0,
        end: 5,
        cols: (0, 5),
        pen: Pen {
            bg: Some(rune::Color::WHITE),
            ..Pen::default()
        },
    });
    let frame = Frame {
        cols: 20,
        rows: 2,
        lines: vec![line, Line::default()],
        ..Frame::default()
    };
    rt.state.term.show_for_test(frame);
    // One frame measures the mono advance, the next draws with it.
    rt.frame(0.0);
    rt.invalidate();
    rt.frame(0.0);

    let row = rt.presented("t0").expect("the row").0;
    let bg = rt.presented("bg0").expect("the background").0;
    let cell = rt.state.term.cell;
    assert!(
        (bg[0] - row[0]).abs() < 0.01,
        "the bar starts at the row's own left edge: {bg:?} vs {row:?}"
    );
    assert!(
        (bg[2] - 5.0 * cell).abs() < 0.01,
        "and covers five cells: {} vs {}",
        bg[2],
        5.0 * cell
    );
    assert!(
        (bg[3] - rt.state.term.line).abs() < 0.01,
        "at the full line height, so bars meet: {bg:?}"
    );
}

#[test]
fn the_pane_draws_below_the_browser_when_it_opens() {
    use crate::tests::helpers::*;
    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    assert!(rt.find("terminal-screen").is_none(), "closed by default");
    assert!(
        rt.find("terminal-grip").is_some(),
        "the handle to open it stays at the bottom"
    );
    assert!(
        rt.find("terminal-header").is_none(),
        "a collapsed pane shows no header"
    );
    assert!(
        rt.find("terminal-divider").is_none(),
        "and no seam line under the handle"
    );

    rt.state.term.open(dir.path().to_path_buf());
    rt.frame(1.0 / 60.0);
    assert!(rt.find("terminal-screen").is_some(), "the pane is laid out");
    assert!(rt.find("terminal-grip").is_some(), "and has its own edge");
    assert!(
        rt.find("files-table").is_some(),
        "the browser stays above it"
    );
}

/// A shape is a rectangle over its cell, painted after the cell's background and under the text:
/// snapped to whole device pixels from the cell's own column, so it abuts its neighbours.
#[cfg(feature = "text-renderer")]
#[test]
fn a_shape_is_a_rectangle_over_its_cell() {
    use crate::tests::helpers::*;
    use rune_tty::grid::Shape;

    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    let mut line = Line {
        text: "  ".to_owned(),
        ..Line::default()
    };
    line.shapes.push(Shape {
        col: 0,
        rect: [0.0, 0.0, 1.0, 1.0],
        fg: rune::Color::WHITE,
    });
    line.shapes.push(Shape {
        col: 1,
        rect: [0.0, 0.0, 0.5, 0.5],
        fg: rune::Color::WHITE,
    });
    let frame = Frame {
        cols: 20,
        rows: 2,
        lines: vec![line, Line::default()],
        ..Frame::default()
    };
    rt.state.term.show_for_test(frame);
    rt.frame(0.0);
    rt.invalidate();
    rt.frame(0.0);

    let row = rt.presented("t0").expect("the row").0;
    let (cell, height) = (rt.state.term.cell, rt.state.term.line);
    let block = rt.presented("sh0_0").expect("the block").0;
    assert!(
        (block[0] - row[0]).abs() < 0.01,
        "flush with the row: {block:?}"
    );
    assert!((block[2] - cell).abs() < 0.01, "one cell wide: {block:?}");
    assert!(
        (block[3] - height).abs() < 0.01,
        "a full line tall, so blocks in a column meet: {block:?}"
    );
    let quadrant = rt.presented("sh1_0").expect("the quadrant").0;
    assert!(
        (quadrant[0] - (row[0] + cell)).abs() < 0.01,
        "the next cell's shape starts on its own column: {quadrant:?}"
    );
    assert!(
        (quadrant[2] - cell * 0.5).abs() < 0.01,
        "half a cell: {quadrant:?}"
    );
    assert!(
        (quadrant[3] - height * 0.5).abs() < 0.01,
        "half a line: {quadrant:?}"
    );
}

/// An update to one cell must not renumber the elements to its right. A background run or a
/// shape inserted on the left of a row changes that cell's rectangle alone; the rest of the row
/// keeps its keys, so Rune updates those rectangles instead of remapping their geometry onto
/// different elements, which read as a flash travelling along the row.
#[cfg(feature = "text-renderer")]
#[test]
fn an_update_keeps_the_elements_to_its_right() {
    use crate::tests::helpers::*;
    use rune_tty::grid::{Pen, Run, Shape};

    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    let mut line = Line {
        text: "     ".to_owned(),
        ..Line::default()
    };
    line.shapes.push(Shape {
        col: 3,
        rect: [0.0, 0.0, 1.0, 1.0],
        fg: rune::Color::WHITE,
    });
    line.shapes.push(Shape {
        col: 4,
        rect: [0.0, 0.0, 0.5, 0.5],
        fg: rune::Color::WHITE,
    });
    line.runs.push(Run {
        start: 3,
        end: 5,
        cols: (3, 5),
        pen: Pen {
            bg: Some(rune::Color::WHITE),
            ..Pen::default()
        },
    });
    let frame = Frame {
        cols: 20,
        rows: 2,
        lines: vec![line, Line::default()],
        ..Frame::default()
    };
    rt.state.term.show_for_test(frame);
    rt.frame(0.0);
    rt.invalidate();
    rt.frame(0.0);

    let row = rt.presented("t0").expect("the row").0;
    let cell = rt.state.term.cell;
    let before = rt.presented("sh3_0").expect("the block at column three").0[0];
    assert!(rt.presented("bg3").is_some(), "its background");

    // A cell to the left gains a shape and a background, as a TUI redraw inserts a box rule
    // and a highlight into a row that is already on screen.
    let line = &mut rt.state.term.frame.lines[0];
    line.shapes.insert(
        0,
        Shape {
            col: 1,
            rect: [0.0, 0.0, 0.5, 1.0],
            fg: rune::Color::WHITE,
        },
    );
    line.runs.insert(
        0,
        Run {
            start: 1,
            end: 2,
            cols: (1, 2),
            pen: Pen {
                bg: Some(rune::Color::WHITE),
                ..Pen::default()
            },
        },
    );
    rt.invalidate();
    rt.frame(0.0);

    let after = rt.presented("sh3_0").expect("the block keeps its key").0[0];
    assert!(
        (after - before).abs() < 0.01,
        "the block left of the update stays where it was: {after} vs {before}"
    );
    assert!(
        rt.presented("sh4_0").is_some(),
        "and so does the next cell's shape"
    );
    let bg = rt.presented("bg3").expect("the background keeps its key").0;
    assert!(
        (bg[0] - (row[0] + 3.0 * cell)).abs() < 0.01,
        "the run still starts on column three: {bg:?}"
    );
    assert!(
        (bg[2] - 2.0 * cell).abs() < 0.01,
        "and still covers two cells: {bg:?}"
    );
    assert!(rt.presented("bg1").is_some(), "the run on the left is new");
    assert!(
        rt.presented("sh1_0").is_some(),
        "and so is the shape beside it"
    );
    assert!(
        rt.presented("text").is_some(),
        "the row's text keeps one identity among its siblings"
    );
}

/// A screen that swaps a block-art logo for a list, the way opencode replaces its welcome logo
/// with the slash-command popover, leaves nothing behind in a transition: no element under the
/// grid is exiting, springing, or below full opacity on the frames that follow.
#[cfg(feature = "text-renderer")]
#[test]
fn a_logo_replaced_by_a_popover_leaves_nothing_fading() {
    use crate::tests::helpers::*;
    use rune_tty::grid::{Pen, Run, Shape};

    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    let mut frame = Frame {
        cols: 40,
        rows: 8,
        ..Frame::default()
    };
    // Four rows of full blocks with a coloured bar: the body of a large logo.
    for _ in 0..4 {
        let mut line = Line {
            text: " ".repeat(40),
            ..Line::default()
        };
        for col in 8..30 {
            line.shapes.push(Shape {
                col,
                rect: [0.0, 0.0, 1.0, 1.0],
                fg: rune::Color::WHITE,
            });
        }
        line.runs.push(Run {
            start: 8,
            end: 30,
            cols: (8, 30),
            pen: Pen {
                bg: Some(rune::Color::WHITE),
                ..Pen::default()
            },
        });
        frame.lines.push(line);
    }
    while frame.lines.len() < 8 {
        frame.lines.push(Line::default());
    }
    rt.state.term.show_for_test(frame);
    for _ in 0..8 {
        rt.frame(1.0 / 60.0);
    }

    // The popover: text rows with one short highlight, no full blocks.
    for (index, line) in rt.state.term.frame.lines.iter_mut().enumerate().take(6) {
        *line = Line {
            text: format!("/command {index}").chars().take(40).collect(),
            ..Line::default()
        };
        let cols = line.text.chars().count() as u16;
        line.runs.push(Run {
            start: 0,
            end: line.text.len(),
            cols: (0, cols),
            pen: Pen::default(),
        });
        if index == 2 {
            line.runs.push(Run {
                start: 0,
                end: line.text.len(),
                cols: (0, cols),
                pen: Pen {
                    bg: Some(rune::Color::WHITE),
                    ..Pen::default()
                },
            });
        }
    }
    rt.invalidate();
    for step in 0..8 {
        rt.frame(1.0 / 60.0);
        for element in rt.elements() {
            if !element.path.contains("terminal-lines") {
                continue;
            }
            assert_eq!(
                element.opacity, 1.0,
                "{} is fading on frame {step}",
                element.path
            );
            assert!(
                !element.exiting,
                "{} is exiting on frame {step}",
                element.path
            );
            assert!(
                !element.moving,
                "{} is still moving on frame {step}",
                element.path
            );
        }
    }
}

/// The engine decides: a cell-filling character becomes shapes on the frame and a space in the
/// line's text, so the columns the runs report are still the cells the shapes cover.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_box_drawing_cell_is_shapes_and_a_space() {
    use rune_tty::engine::Engine;

    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        20,
        4,
        (7.0, 15.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("an engine");
    engine.feed("\x1b[2J\x1b[H┌─┐\r\n│█│\r\n└─┘".as_bytes());
    let frame = engine.frame();

    for (index, row) in ["   ", "   ", "   "].iter().enumerate() {
        assert_eq!(
            frame.lines[index].text.as_str(),
            *row,
            "row {index} is spaces"
        );
        let mut cols: Vec<u16> = frame.lines[index].shapes.iter().map(|s| s.col).collect();
        cols.dedup();
        assert_eq!(cols, [0, 1, 2], "and covers every cell on row {index}");
    }
    let block = frame.lines[1]
        .shapes
        .iter()
        .find(|shape| shape.col == 1)
        .expect("the middle cell");
    assert_eq!(
        block.rect,
        [0.0, 0.0, 1.0, 1.0],
        "a full block is the whole cell"
    );
    assert!(
        frame.lines[0]
            .shapes
            .iter()
            .all(|shape| shape.rect[1] <= 0.5 && shape.rect[3] >= 0.5),
        "a rule runs through the middle of its row, the way a box drawn in a terminal does"
    );
    assert_eq!(
        (frame.lines[0].runs[0].start, frame.lines[0].runs[0].cols),
        (0, (0, 3)),
        "the space still maps one byte to one cell, so the runs stay honest"
    );
}

/// The desktop default streams the grid as one image: one texture sized to the viewport and its
/// two overscan rows, with no row elements behind it.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
#[test]
fn the_grid_streams_as_one_texture_by_default() {
    use crate::tests::helpers::*;
    use rune_tty::grid::{Frame, Line};

    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    let frame = Frame {
        cols: 20,
        rows: 2,
        lines: vec![
            Line {
                text: "hello".to_owned(),
                ..Line::default()
            },
            Line::default(),
        ],
        ..Frame::default()
    };
    rt.state.term.show_for_test(frame);
    rt.frame(0.0);

    let grid = rt
        .presented("terminal-texture")
        .expect("the streamed grid")
        .0;
    let (cell, line) = (rt.state.term.cell, rt.state.term.line);
    assert!(
        (grid[2] - 20.0 * cell).abs() < 0.01,
        "the image covers the grid's columns: {grid:?}"
    );
    assert!(
        (grid[3] - 4.0 * line).abs() < 0.01,
        "and the viewport plus one overscan row each side: {grid:?}"
    );
    assert!(
        rt.presented("t0").is_none(),
        "the default builds no row elements"
    );
}

/// A notice or the starting state is still text: there is no frame to stream yet.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
#[test]
fn a_notice_or_a_starting_shell_still_uses_text() {
    use crate::tests::helpers::*;
    use rune_tty::grid::Frame;

    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    rt.state.term.show_for_test(Frame::default());
    rt.frame(0.0);
    assert!(rt.find("terminal-starting").is_some(), "the starting note");
    assert!(
        rt.presented("terminal-texture").is_none(),
        "no image to stream yet"
    );

    rt.state.term.notice = Some("The terminal is unavailable here.".to_owned());
    rt.invalidate();
    rt.frame(0.0);
    assert!(rt.find("terminal-notice").is_some(), "the notice");
    assert!(
        rt.presented("terminal-texture").is_none(),
        "a notice is text, not a grid"
    );
}
