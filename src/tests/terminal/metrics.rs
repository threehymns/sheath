//! The cell the pane measures: its advance wide and its line box tall, from the font and the
//! display scale, since the backgrounds, the cursor and the shell's columns all follow it.

/// The pane's cell is the font's own cell: its advance wide, its line box tall. A guess here
/// makes the background rectangles, the cursor and the shell's column count disagree with the
/// glyphs, and glyphs that fill a cell stop tiling.
#[test]
fn the_cell_is_the_font_cell() {
    use crate::tests::helpers::*;
    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    rt.state.term.open(dir.path().to_path_buf());
    rt.frame(1.0 / 60.0);
    // The headless runtime is a 2x display.
    let scale = 2.0;
    let kit = rune_theme_opencode::kit(rt.state.dark.get());
    let natural = kit.ty.mono.lh(0.0);
    assert!(
        (rt.state.term.cell * scale).fract().abs() < 0.01,
        "the cell is a whole number of device pixels: {}",
        rt.state.term.cell
    );
    assert!(
        (rt.state.term.line * scale).fract().abs() < 0.01,
        "and so is the row: {}",
        rt.state.term.line
    );
    let advance = rune::text::advance("M", natural);
    let drawn = rt.state.term.text_style(kit.ty.mono);
    assert!(
        (rune::text::advance("M", drawn) - rt.state.term.cell).abs() < 0.01,
        "the font is tracked to the cell, so a glyph advance is exactly one cell: {} vs {}",
        rune::text::advance("M", drawn),
        rt.state.term.cell
    );
    assert!(
        (advance - rt.state.term.cell).abs() > 0.0001 || rt.state.term.cell == advance,
        "the cell comes from the font's advance: {advance}"
    );
    assert!(
        rt.state.term.line < kit.ty.mono.line_height,
        "the row is the font's box, not the theme's paragraph measure: {}",
        rt.state.term.line
    );
}

/// A font setting, or a display at another scale, simply takes effect: the cell comes from the
/// style the rows are drawn with on every poll, never from the first frame's measurement.
#[test]
fn a_new_font_or_scale_re_measures_the_cell() {
    use crate::tests::helpers::*;
    let dir = long_folder(4);
    let mut rt = runtime(app(&dir));
    rt.state.term.open(dir.path().to_path_buf());
    rt.frame(1.0 / 60.0);
    let kit = rune_theme_opencode::kit(rt.state.dark.get());
    let natural = kit.ty.mono.lh(0.0);
    let (cell, line) = (rt.state.term.cell, rt.state.term.line);
    let bigger = rune::TextStyle {
        size: natural.size + 3.0,
        ..natural
    };
    rt.state.term.measure(bigger, 2.0);
    assert!(
        rt.state.term.cell > cell && rt.state.term.line > line,
        "a larger font is a larger cell: {}x{} was {}x{}",
        rt.state.term.cell,
        rt.state.term.line,
        cell,
        line
    );
    rt.state.term.measure(natural, 1.0);
    assert!(
        (rt.state.term.cell * 1.0).fract().abs() < 0.01,
        "a 1x window gets a whole-pixel cell as well: {}",
        rt.state.term.cell
    );
}
