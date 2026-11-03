//! Which rectangles each code point is drawn as: the stroke table for the box-drawing range, the
//! arcs for the rounded corners, and the blocks, halves, quadrants, sextants and octants. Pure
//! data — the geometry that turns it into rectangles is in the parent module.

use super::Shapes;

/// Which way a rule leaves the cell, and how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stroke {
    Blank,
    Light,
    Heavy,
    /// Two light rules either side of the centreline.
    Double,
    /// A dashed rule: `count` dashes across the cell, in the given weight.
    Dash(u8, bool),
}

/// The centre of the quarter arc a rounded corner turns on, and the quarter it starts in.
///
/// The arc is half a cell's radius about the cell's far corner, so it is tangent to the rules it
/// joins and the border curves through the cell instead of turning a square corner in its
/// middle. `start` counts quarter turns clockwise from the top edge.
pub(super) fn rounded(c: char) -> Option<((f32, f32), u8)> {
    Some(match c {
        '\u{256D}' => ((1.0, 1.0), 2), // arc down and right
        '\u{256E}' => ((0.0, 1.0), 3), // arc down and left
        '\u{256F}' => ((0.0, 0.0), 0), // arc up and left
        '\u{2570}' => ((1.0, 0.0), 1), // arc up and right
        _ => return None,
    })
}

/// The strokes leaving each cell edge, in the order up, down, left, right. `None` means the
/// character is not drawn from the cell — shades, diagonals and everything outside these ranges
/// keep the font's glyph.
pub(super) fn strokes(c: char) -> Option<[Stroke; 4]> {
    use Stroke::{Blank, Dash, Double, Heavy, Light};
    Some(match c {
        // Dashes.
        '\u{2500}' => [Blank, Blank, Light, Light],
        '\u{2501}' => [Blank, Blank, Heavy, Heavy],
        '\u{2502}' => [Light, Light, Blank, Blank],
        '\u{2503}' => [Heavy, Heavy, Blank, Blank],
        '\u{2504}' => [Blank, Blank, Dash(3, false), Dash(3, false)],
        '\u{2505}' => [Blank, Blank, Dash(3, true), Dash(3, true)],
        '\u{2506}' => [Dash(3, false), Dash(3, false), Blank, Blank],
        '\u{2507}' => [Dash(3, true), Dash(3, true), Blank, Blank],
        '\u{2508}' => [Blank, Blank, Dash(4, false), Dash(4, false)],
        '\u{2509}' => [Blank, Blank, Dash(4, true), Dash(4, true)],
        '\u{250A}' => [Dash(4, false), Dash(4, false), Blank, Blank],
        '\u{250B}' => [Dash(4, true), Dash(4, true), Blank, Blank],
        // Corners.
        '\u{250C}' => [Blank, Light, Blank, Light],
        '\u{250D}' => [Blank, Light, Blank, Heavy],
        '\u{250E}' => [Blank, Heavy, Blank, Light],
        '\u{250F}' => [Blank, Heavy, Blank, Heavy],
        '\u{2510}' => [Blank, Light, Light, Blank],
        '\u{2511}' => [Blank, Light, Heavy, Blank],
        '\u{2512}' => [Blank, Heavy, Light, Blank],
        '\u{2513}' => [Blank, Heavy, Heavy, Blank],
        '\u{2514}' => [Light, Blank, Blank, Light],
        '\u{2515}' => [Light, Blank, Blank, Heavy],
        '\u{2516}' => [Heavy, Blank, Blank, Light],
        '\u{2517}' => [Heavy, Blank, Blank, Heavy],
        '\u{2518}' => [Light, Blank, Light, Blank],
        '\u{2519}' => [Light, Blank, Heavy, Blank],
        '\u{251A}' => [Heavy, Blank, Light, Blank],
        '\u{251B}' => [Heavy, Blank, Heavy, Blank],
        // Left Ts.
        '\u{251C}' => [Light, Light, Blank, Light],
        '\u{251D}' => [Light, Light, Blank, Heavy],
        '\u{251E}' => [Heavy, Light, Blank, Light],
        '\u{251F}' => [Light, Heavy, Blank, Light],
        '\u{2520}' => [Heavy, Heavy, Blank, Light],
        '\u{2521}' => [Heavy, Light, Blank, Heavy],
        '\u{2522}' => [Light, Heavy, Blank, Heavy],
        '\u{2523}' => [Heavy, Heavy, Blank, Heavy],
        // Right Ts.
        '\u{2524}' => [Light, Light, Light, Blank],
        '\u{2525}' => [Light, Light, Heavy, Blank],
        '\u{2526}' => [Heavy, Light, Light, Blank],
        '\u{2527}' => [Light, Heavy, Light, Blank],
        '\u{2528}' => [Heavy, Heavy, Light, Blank],
        '\u{2529}' => [Heavy, Light, Heavy, Blank],
        '\u{252A}' => [Light, Heavy, Heavy, Blank],
        '\u{252B}' => [Heavy, Heavy, Heavy, Blank],
        // Down Ts.
        '\u{252C}' => [Blank, Light, Light, Light],
        '\u{252D}' => [Blank, Light, Heavy, Light],
        '\u{252E}' => [Blank, Light, Light, Heavy],
        '\u{252F}' => [Heavy, Light, Heavy, Heavy],
        '\u{2530}' => [Light, Heavy, Light, Light],
        '\u{2531}' => [Heavy, Heavy, Heavy, Light],
        '\u{2532}' => [Heavy, Heavy, Light, Heavy],
        '\u{2533}' => [Heavy, Heavy, Heavy, Heavy],
        // Up Ts.
        '\u{2534}' => [Light, Blank, Light, Light],
        '\u{2535}' => [Light, Blank, Heavy, Light],
        '\u{2536}' => [Light, Blank, Light, Heavy],
        '\u{2537}' => [Light, Blank, Heavy, Heavy],
        '\u{2538}' => [Heavy, Blank, Light, Light],
        '\u{2539}' => [Heavy, Blank, Heavy, Light],
        '\u{253A}' => [Heavy, Blank, Light, Heavy],
        '\u{253B}' => [Heavy, Blank, Heavy, Heavy],
        // Crosses.
        '\u{253C}' => [Light, Light, Light, Light],
        '\u{253D}' => [Light, Light, Heavy, Light],
        '\u{253E}' => [Light, Light, Light, Heavy],
        '\u{253F}' => [Light, Light, Heavy, Heavy],
        '\u{2540}' => [Heavy, Light, Light, Light],
        '\u{2541}' => [Light, Heavy, Light, Light],
        '\u{2542}' => [Heavy, Heavy, Light, Light],
        '\u{2543}' => [Heavy, Light, Heavy, Light],
        '\u{2544}' => [Heavy, Light, Light, Heavy],
        '\u{2545}' => [Light, Heavy, Heavy, Light],
        '\u{2546}' => [Light, Heavy, Light, Heavy],
        '\u{2547}' => [Heavy, Light, Heavy, Heavy],
        '\u{2548}' => [Light, Heavy, Heavy, Heavy],
        '\u{2549}' => [Heavy, Heavy, Heavy, Light],
        '\u{254A}' => [Heavy, Heavy, Light, Heavy],
        '\u{254B}' => [Heavy, Heavy, Heavy, Heavy],
        // Double dashes.
        '\u{254C}' => [Blank, Blank, Dash(2, false), Dash(2, false)],
        '\u{254D}' => [Blank, Blank, Dash(2, true), Dash(2, true)],
        '\u{254E}' => [Dash(2, false), Dash(2, false), Blank, Blank],
        '\u{254F}' => [Dash(2, true), Dash(2, true), Blank, Blank],
        // Double lines.
        '\u{2550}' => [Blank, Blank, Double, Double],
        '\u{2551}' => [Double, Double, Blank, Blank],
        '\u{2552}' => [Blank, Light, Blank, Double],
        '\u{2553}' => [Blank, Double, Blank, Light],
        '\u{2554}' => [Blank, Double, Blank, Double],
        '\u{2555}' => [Blank, Light, Double, Blank],
        '\u{2556}' => [Blank, Double, Light, Blank],
        '\u{2557}' => [Blank, Double, Double, Blank],
        '\u{2558}' => [Light, Blank, Blank, Double],
        '\u{2559}' => [Double, Blank, Blank, Light],
        '\u{255A}' => [Double, Blank, Blank, Double],
        '\u{255B}' => [Light, Blank, Double, Blank],
        '\u{255C}' => [Double, Blank, Light, Blank],
        '\u{255D}' => [Double, Blank, Double, Blank],
        '\u{255E}' => [Light, Light, Blank, Double],
        '\u{255F}' => [Double, Double, Blank, Light],
        '\u{2560}' => [Double, Double, Blank, Double],
        '\u{2561}' => [Light, Light, Double, Blank],
        '\u{2562}' => [Double, Double, Light, Blank],
        '\u{2563}' => [Double, Double, Double, Blank],
        '\u{2564}' => [Blank, Light, Double, Double],
        '\u{2565}' => [Blank, Double, Light, Light],
        '\u{2566}' => [Blank, Double, Double, Double],
        '\u{2567}' => [Light, Blank, Double, Double],
        '\u{2568}' => [Double, Blank, Light, Light],
        '\u{2569}' => [Double, Blank, Double, Double],
        '\u{256A}' => [Light, Light, Double, Double],
        '\u{256B}' => [Double, Double, Light, Light],
        '\u{256C}' => [Double, Double, Double, Double],
        // The rounded corners (U+256D..=U+2570) are drawn as arcs, see `rounded`.
        // Stubs.
        '\u{2574}' => [Blank, Blank, Light, Blank],
        '\u{2575}' => [Light, Blank, Blank, Blank],
        '\u{2576}' => [Blank, Blank, Blank, Light],
        '\u{2577}' => [Blank, Light, Blank, Blank],
        '\u{2578}' => [Blank, Blank, Heavy, Blank],
        '\u{2579}' => [Heavy, Blank, Blank, Blank],
        '\u{257A}' => [Blank, Blank, Blank, Heavy],
        '\u{257B}' => [Blank, Heavy, Blank, Blank],
        '\u{257C}' => [Blank, Blank, Light, Heavy],
        '\u{257D}' => [Light, Heavy, Blank, Blank],
        '\u{257E}' => [Blank, Blank, Heavy, Light],
        '\u{257F}' => [Heavy, Light, Blank, Blank],
        _ => return None,
    })
}

/// The blocks, halves and quadrants of U+2580..=U+259F, and the sextants and octants that divide
/// the cell more finely: rectangles in cell fractions. The shades (U+2591..=U+2593) are left to
/// the font for now.
pub(super) fn block(c: char) -> Option<Shapes> {
    if let Some((mask, rows)) = mosaic(c) {
        return Some(pieces(mask, rows));
    }
    let mut out = Shapes::new();
    // A legend quadrant is a union of quadrants, so it is one to three rectangles rather than a
    // table of its own.
    let (top, bottom) = ((0.0, 0.5), (0.5, 1.0));
    let (left, right) = ((0.0, 0.5), (0.5, 1.0));
    match c {
        '\u{2580}' => out.push([0.0, 0.0, 1.0, 0.5]),
        '\u{2581}' => out.push([0.0, 0.875, 1.0, 1.0]),
        '\u{2582}' => out.push([0.0, 0.75, 1.0, 1.0]),
        '\u{2583}' => out.push([0.0, 0.625, 1.0, 1.0]),
        '\u{2584}' => out.push([0.0, 0.5, 1.0, 1.0]),
        '\u{2585}' => out.push([0.0, 0.375, 1.0, 1.0]),
        '\u{2586}' => out.push([0.0, 0.25, 1.0, 1.0]),
        '\u{2587}' => out.push([0.0, 0.125, 1.0, 1.0]),
        '\u{2588}' => out.push([0.0, 0.0, 1.0, 1.0]),
        '\u{2589}' => out.push([0.0, 0.0, 0.875, 1.0]),
        '\u{258A}' => out.push([0.0, 0.0, 0.75, 1.0]),
        '\u{258B}' => out.push([0.0, 0.0, 0.625, 1.0]),
        '\u{258C}' => out.push([0.0, 0.0, 0.5, 1.0]),
        '\u{258D}' => out.push([0.0, 0.0, 0.375, 1.0]),
        '\u{258E}' => out.push([0.0, 0.0, 0.25, 1.0]),
        '\u{258F}' => out.push([0.0, 0.0, 0.125, 1.0]),
        '\u{2590}' => out.push([0.5, 0.0, 1.0, 1.0]),
        '\u{2594}' => out.push([0.0, 0.0, 1.0, 0.125]),
        '\u{2595}' => out.push([0.875, 0.0, 1.0, 1.0]),
        '\u{2596}' => out.push([left.0, bottom.0, left.1, bottom.1]),
        '\u{2597}' => out.push([right.0, bottom.0, right.1, bottom.1]),
        '\u{2598}' => out.push([left.0, top.0, left.1, top.1]),
        '\u{2599}' => {
            out.push([left.0, top.0, left.1, bottom.1]);
            out.push([right.0, bottom.0, right.1, bottom.1]);
        }
        '\u{259A}' => {
            out.push([left.0, top.0, left.1, top.1]);
            out.push([right.0, bottom.0, right.1, bottom.1]);
        }
        '\u{259B}' => {
            out.push([left.0, top.0, right.1, top.1]);
            out.push([left.0, bottom.0, left.1, bottom.1]);
        }
        '\u{259C}' => {
            out.push([left.0, top.0, right.1, top.1]);
            out.push([right.0, bottom.0, right.1, bottom.1]);
        }
        '\u{259D}' => out.push([right.0, top.0, right.1, top.1]),
        '\u{259E}' => {
            out.push([right.0, top.0, right.1, top.1]);
            out.push([left.0, bottom.0, left.1, bottom.1]);
        }
        '\u{259F}' => {
            out.push([right.0, top.0, right.1, bottom.1]);
            out.push([left.0, bottom.0, left.1, bottom.1]);
        }
        _ => return None,
    }
    Some(out)
}

/// The sextants of U+1FB00..=U+1FB3B and the octants of U+1CD00..=U+1CDE5, as bit masks: bit
/// `2n` is the left half of row `n` counted from the top, bit `2n+1` its right half. That is the
/// numbering Unicode names them by — `BLOCK SEXTANT-13` is bits 0 and 2, `BLOCK OCTANT-2345678`
/// every bit but 0 — and each table is indexed by the offset from the first code point, so
/// `SEXTANTS[0]` is U+1FB00, the upper-left sixth. A combination an older character already
/// draws (the full block, halves, quarters) has no code point of its own and so no entry here.
const SEXTANTS: [u8; 60] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21,
    0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31, 0x32,
    0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e,
];

const OCTANTS: [u8; 230] = [
    0x04, 0x06, 0x07, 0x08, 0x09, 0x0b, 0x0c, 0x0d, 0x0e, 0x10, 0x11, 0x12, 0x13, 0x15, 0x16, 0x17,
    0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
    0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
    0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a,
    0x4b, 0x4c, 0x4d, 0x4e, 0x4f, 0x51, 0x52, 0x53, 0x54, 0x56, 0x57, 0x58, 0x59, 0x5b, 0x5c, 0x5d,
    0x5e, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e,
    0x6f, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e,
    0x7f, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f,
    0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f,
    0xa1, 0xa2, 0xa3, 0xa4, 0xa6, 0xa7, 0xa8, 0xa9, 0xab, 0xac, 0xad, 0xae, 0xb0, 0xb1, 0xb2, 0xb3,
    0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf, 0xc1, 0xc2, 0xc3, 0xc4,
    0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xcb, 0xcc, 0xcd, 0xce, 0xcf, 0xd0, 0xd1, 0xd2, 0xd3, 0xd4,
    0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xdb, 0xdc, 0xdd, 0xde, 0xdf, 0xe0, 0xe1, 0xe2, 0xe3, 0xe4,
    0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xeb, 0xec, 0xed, 0xee, 0xef, 0xf1, 0xf2, 0xf3, 0xf4, 0xf6,
    0xf7, 0xf8, 0xf9, 0xfb, 0xfd, 0xfe,
];

/// The mask and the number of rows a sextant or octant stands for; `None` for every other
/// character, which keeps its glyph.
fn mosaic(c: char) -> Option<(u8, u8)> {
    let cp = c as u32;
    if (0x1FB00..=0x1FB3B).contains(&cp) {
        return Some((SEXTANTS[(cp - 0x1FB00) as usize], 3));
    }
    (0x1CD00..=0x1CDE5)
        .contains(&cp)
        .then(|| (OCTANTS[(cp - 0x1CD00) as usize], 4))
}

/// The rectangles a mask fills: two columns across and `rows` down, one rectangle per set bit, so
/// a nearly full cell is a handful of them rather than a bitmap. Every edge is an exact fraction
/// of the cell, so neighbouring pieces meet without a seam at any size.
fn pieces(mask: u8, rows: u8) -> Shapes {
    let mut out = Shapes::new();
    for bit in 0..rows * 2 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let column = bit % 2;
        let row = bit / 2;
        out.push([
            f32::from(column) / 2.0,
            f32::from(row) / f32::from(rows),
            f32::from(column + 1) / 2.0,
            f32::from(row + 1) / f32::from(rows),
        ]);
    }
    out
}
