//! The tables behind the cell-filling characters are pure functions of the code point
//! and the cell's size, so they can be checked without a terminal at all.

use crate::term::shapes::{self, Metrics};

/// An IBM Plex Mono cell at 2x, the size the pane measures on the headless display.
fn metrics() -> Metrics {
    Metrics {
        cell: 7.0,
        line: 15.0,
        scale: 2.0,
    }
}

fn rects(c: char) -> Vec<[f32; 4]> {
    let shapes = shapes::cell(c, metrics()).unwrap_or_else(|| panic!("{c} is drawn from a shape"));
    (0..shapes.len()).map(|index| shapes.get(index)).collect()
}

/// The bounding box of a character's rectangles, which for a block is also its area.
fn union(rects: &[[f32; 4]]) -> [f32; 4] {
    rects.iter().fold(
        [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
        |u: [f32; 4], r: &[f32; 4]| {
            [
                u[0].min(r[0]),
                u[1].min(r[1]),
                u[2].max(r[2]),
                u[3].max(r[3]),
            ]
        },
    )
}

/// A rounded corner: the character, and the points on the cell's edges its arc leaves and
/// arrives at.
type Corner = (char, (f32, f32), (f32, f32));

fn covers(rects: &[[f32; 4]], x: f32, y: f32) -> bool {
    rects
        .iter()
        .any(|r| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3])
}

/// A block fills the part of its cell it stands for and no more: a half is half the cell, a
/// quadrant a quarter, a legend quadrant the pair of quadrants it names.
#[test]
fn a_block_covers_its_cell_exactly() {
    let cases: [(char, [f32; 4]); 17] = [
        ('█', [0.0, 0.0, 1.0, 1.0]),
        ('▀', [0.0, 0.0, 1.0, 0.5]),
        ('▄', [0.0, 0.5, 1.0, 1.0]),
        ('▌', [0.0, 0.0, 0.5, 1.0]),
        ('▐', [0.5, 0.0, 1.0, 1.0]),
        ('▔', [0.0, 0.0, 1.0, 0.125]),
        ('▁', [0.0, 0.875, 1.0, 1.0]),
        ('▏', [0.0, 0.0, 0.125, 1.0]),
        ('▕', [0.875, 0.0, 1.0, 1.0]),
        ('▘', [0.0, 0.0, 0.5, 0.5]),
        ('▝', [0.5, 0.0, 1.0, 0.5]),
        ('▖', [0.0, 0.5, 0.5, 1.0]),
        ('▗', [0.5, 0.5, 1.0, 1.0]),
        ('▚', [0.0, 0.0, 1.0, 1.0]),
        ('▞', [0.0, 0.0, 1.0, 1.0]),
        ('▛', [0.0, 0.0, 1.0, 1.0]),
        ('▟', [0.0, 0.0, 1.0, 1.0]),
    ];
    for (c, want) in cases {
        let got = rects(c);
        assert_eq!(union(&got), want, "{c}");
        for rect in &got {
            assert!(
                rect[0] >= 0.0 && rect[1] >= 0.0 && rect[2] <= 1.0 && rect[3] <= 1.0,
                "{c} stays inside its cell: {rect:?}"
            );
        }
    }
    assert!(covers(&rects('▚'), 0.1, 0.1), "upper left");
    assert!(covers(&rects('▚'), 0.9, 0.9), "lower right");
    assert!(!covers(&rects('▚'), 0.9, 0.1), "but not the other two");
}

/// A rule leaves its cell at the cell's own edges, so two neighbouring cells' rules meet at
/// the boundary instead of leaving a gap. It runs down the middle of the cell, the way a
/// box drawn in a terminal does.
#[test]
fn a_rule_leaves_from_the_cell_edges() {
    let across = rects('─');
    let span = union(&across);
    assert_eq!((span[0], span[2]), (0.0, 1.0), "spans every column");
    for rect in &across {
        assert!(
            rect[1] > 0.4 && rect[3] < 0.6,
            "a horizontal rule is halfway down the row: {rect:?}"
        );
    }
    let down = rects('│');
    assert_eq!(union(&down)[1], 0.0);
    assert_eq!(union(&down)[3], 1.0, "spans every row");
    for rect in &down {
        assert!(
            rect[0] > 0.4 && rect[2] < 0.6,
            "a vertical rule is halfway across the cell: {rect:?}"
        );
    }
    // ┼ reaches all four edges, so the four cells around it join it in every direction.
    assert_eq!(union(&rects('┼')), [0.0, 0.0, 1.0, 1.0]);
    assert!(covers(&rects('┼'), 0.5, 0.5), "and the crossing is solid");
}

/// A light stroke is one device pixel, a heavy one two: the geometry takes them from the
/// display scale, not from the font.
#[test]
fn a_stroke_is_a_device_pixel_wide() {
    let m = metrics();
    let light = 1.0 / (m.scale * m.line);
    let heavy = 2.0 / (m.scale * m.line);
    let thin = rects('─')[0];
    assert!(((thin[3] - thin[1]) - light).abs() < 1e-6, "{thin:?}");
    let thick = rects('━')[0];
    assert!(((thick[3] - thick[1]) - heavy).abs() < 1e-6, "{thick:?}");
    let vertical = 1.0 / (m.scale * m.cell);
    let thin = rects('│')[0];
    assert!(((thin[2] - thin[0]) - vertical).abs() < 1e-6, "{thin:?}");
}

/// The two strokes of a double rule straddle the centre, and a crossing leaves the middle
/// open: a double cross is four lines, not a filled square.
#[test]
fn a_double_cross_leaves_its_centre_open() {
    let cross = rects('╬');
    assert_eq!(union(&cross), [0.0, 0.0, 1.0, 1.0]);
    assert!(
        !covers(&cross, 0.5, 0.5),
        "the four double lines meet without filling the middle"
    );
    let parallel = rects('═');
    let span = union(&parallel);
    assert_eq!(
        (span[0], span[2]),
        (0.0, 1.0),
        "a double horizontal spans every column"
    );
    assert!(
        !covers(&parallel, 0.5, 0.5),
        "and a double horizontal is two lines"
    );
    assert!(
        covers(
            &parallel,
            0.5,
            0.5 - 1.0 / (metrics().scale * metrics().line)
        ),
        "one of the pair is just above the centreline"
    );
}

/// A rounded corner is a curve: it leaves through the two edges the neighbouring cells' rules
/// use, and it does not turn a square corner in the middle of the cell the way the square
/// corners do.
#[test]
fn a_rounded_corner_curves_through_the_cell() {
    let cases: [Corner; 4] = [
        ('╭', (1.0, 0.5), (0.5, 1.0)),
        ('╮', (0.0, 0.5), (0.5, 1.0)),
        ('╯', (0.0, 0.5), (0.5, 0.0)),
        ('╰', (1.0, 0.5), (0.5, 0.0)),
    ];
    for (c, leaves, arrives) in cases {
        let arc = rects(c);
        assert!(
            arc.len() > 2,
            "{c} is a curve, not one corner rect: {arc:?}"
        );
        assert!(covers(&arc, leaves.0, leaves.1), "{c} leaves at {leaves:?}");
        assert!(
            covers(&arc, arrives.0, arrives.1),
            "{c} arrives at {arrives:?}"
        );
        assert!(
            !covers(&arc, 0.5, 0.5),
            "{c} does not turn a square corner in the middle of the cell"
        );
        for rect in &arc {
            assert!(
                rect[0] >= 0.0 && rect[1] >= 0.0 && rect[2] <= 1.0 && rect[3] <= 1.0,
                "{c} stays inside its cell: {rect:?}"
            );
        }
    }
    assert!(
        covers(&rects('┌'), 0.5, 0.5),
        "while the square corner does turn in the middle"
    );
}

/// Everything outside the cell-filling ranges keeps its glyph: the font draws it, the pane
/// adds nothing underneath.
#[test]
fn only_cell_filling_characters_get_shapes() {
    for c in ['a', ' ', 'M', '\u{2571}'] {
        assert!(
            shapes::cell(c, metrics()).is_none(),
            "{c} is drawn from the font"
        );
    }
    for c in ['─', '█', '▘', '╭', '┄'] {
        assert!(
            shapes::cell(c, metrics()).is_some(),
            "{c} is drawn from its cell"
        );
    }
}

/// The sextants and octants divide the cell into sixths and eighths, which is what the finer
/// image blitters draw with (notcurses' `3x2` and `4x2`): every piece is the same size, so a
/// character covers exactly the sub-cells its Unicode name lists, and none leaves the cell.
#[test]
fn a_sextant_and_an_octant_tile_their_cell() {
    for (first, count, rows) in [(0x1FB00u32, 60u32, 3u32), (0x1CD00, 230, 4)] {
        for cp in first..first + count {
            let c = char::from_u32(cp).expect("a printable code point");
            let pieces = rects(c);
            let want = f64::from(pieces.len() as u32) / f64::from(rows) / 2.0;
            let area: f64 = pieces
                .iter()
                .map(|r| f64::from(r[2] - r[0]) * f64::from(r[3] - r[1]))
                .sum();
            assert!(
                (area - want).abs() < 1e-5,
                "{cp:#x} covers {area} of the cell in {} pieces",
                pieces.len()
            );
            for r in &pieces {
                assert!(
                    r[0] >= 0.0 && r[1] >= 0.0 && r[2] <= 1.0 && r[3] <= 1.0,
                    "{cp:#x} stays inside its cell: {r:?}"
                );
            }
        }
    }
    // U+1FB00 names the upper-left sixth and U+1FB3B every sixth but that one. The octants count
    // the same way, so U+1CD00 is the middle-left eighth and U+1CDE5 all but the upper-left one.
    assert_eq!(rects('\u{1FB00}'), vec![[0.0, 0.0, 0.5, 1.0 / 3.0]]);
    let five = rects('\u{1FB3B}');
    assert_eq!(five.len(), 5);
    assert!(!covers(&five, 0.1, 0.1), "the upper-left sixth is empty");
    assert!(covers(&five, 0.9, 0.1) && covers(&five, 0.1, 0.9));
    assert_eq!(rects('\u{1CD00}'), vec![[0.0, 0.25, 0.5, 0.5]]);
    let seven = rects('\u{1CDE5}');
    assert_eq!(seven.len(), 7);
    assert!(!covers(&seven, 0.1, 0.1), "the upper-left octant is empty");
    assert!(covers(&seven, 0.1, 0.3));
}
