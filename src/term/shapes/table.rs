//! Which rectangles each code point is drawn as: the stroke table for the box-drawing range, the
//! arcs for the rounded corners, and the blocks, halves and quadrants. Pure data — the geometry
//! that turns it into rectangles is in the parent module.

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

/// The blocks, halves and quadrants of U+2580..=U+259F: rectangles in cell fractions. The shades
/// (U+2591..=U+2593) are left to the font for now.
pub(super) fn block(c: char) -> Option<Shapes> {
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
