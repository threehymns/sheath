//! What a program sees when it asks the terminal what it can do. A program that draws pixels
//! (notcurses, image viewers) believes only what these answers say, so a missing or zero answer
//! is what turns a blitter into "not supported".

#![cfg(not(target_arch = "wasm32"))]

use rune_tty::engine::Engine;

fn engine(cols: u16, rows: u16) -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let engine = Engine::spawn(
        dir.path(),
        cols,
        rows,
        (8.0, 16.0),
        1.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    (dir, engine)
}

fn ask(engine: &mut Engine, query: &[u8]) -> String {
    String::from_utf8_lossy(&engine.feed(query)).into_owned()
}

/// A graphics program asks for the cell and screen size in pixels before it will place an image:
/// a zero answer is how a terminal says it has no pixel grid at all. The pane knows both, so it
/// answers with the grid it draws.
#[test]
fn the_pixel_geometry_a_graphics_program_asks_for_is_answered() {
    let (_dir, mut engine) = engine(80, 24);
    // CSI 16 t → CSI 6 ; height ; width t, one cell in pixels.
    let cell = ask(&mut engine, b"\x1b[16t");
    assert_eq!(cell, "\x1b[6;16;8t", "one cell at the spawn's cell size");
    // CSI 14 t → CSI 4 ; height ; width t, the whole grid in pixels.
    let screen = ask(&mut engine, b"\x1b[14t");
    assert_eq!(
        screen, "\x1b[4;384;640t",
        "24 rows of 16 by 80 columns of 8"
    );
    // CSI 18 t → CSI 8 ; rows ; columns t, the grid in cells.
    assert_eq!(ask(&mut engine, b"\x1b[18t"), "\x1b[8;24;80t");
}

/// A program asks what terminal it is talking to and picks its behaviour from the name: notcurses
/// recognizes `ghostty ` and enables the drawing and pixel support that goes with it, so the pane
/// answers as Ghostty rather than as the core library it is built on.
#[test]
fn the_terminal_names_itself_as_ghostty() {
    let (_dir, mut engine) = engine(80, 24);
    let reply = ask(&mut engine, b"\x1b[>q");
    assert!(
        reply.starts_with("\x1bP>|ghostty "),
        "the XTVERSION name programs match on: {reply:?}"
    );
    assert!(reply.ends_with("\x1b\\"), "a string terminated by ST");
}

/// The kitty graphics query is how a program decides whether it can send images at all: it asks
/// the terminal to load a one pixel image and report whether that worked, without storing it.
#[test]
fn the_kitty_graphics_query_is_answered() {
    let (_dir, mut engine) = engine(80, 24);
    let reply = ask(&mut engine, b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\");
    assert_eq!(reply, "\x1b_Gi=31;OK\x1b\\", "the query reports success");
    assert!(
        engine.frame().images.is_empty(),
        "a query transmits without displaying"
    );
}

/// Every answer a terminal gives to a capability probe, printed once. Run it with `--nocapture`
/// when a program reports something as unavailable: this is the set of facts it had.
#[test]
#[ignore = "manual probe; prints the answers a program sees"]
fn probe_capability_answers() {
    let (_dir, mut engine) = engine(80, 24);
    for (name, query) in [
        ("DA1", &b"\x1b[c"[..]),
        ("DA2", b"\x1b[>c"),
        ("XTVERSION", b"\x1b[>q"),
        ("cell px (16t)", b"\x1b[16t"),
        ("screen px (14t)", b"\x1b[14t"),
        ("grid cells (18t)", b"\x1b[18t"),
        ("kitty query", b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\"),
        ("notcurses kitty query", b"\x1b_Gi=1,a=q;\x1b\\"),
        ("kitty feature query", b"\x1b_Ga=q,i=32,e=T\x1b\\"),
        ("kitty keyboard push", b"\x1b[=27u"),
        ("kitty keyboard query", b"\x1b[?u"),
        ("sixel DA1", b"\x1b[c"),
        ("sync output (2026)", b"\x1b[?2026$p"),
        ("kitty keyboard", b"\x1b[?u"),
        ("bg colour (OSC 11)", b"\x1b]11;?\x1b\\"),
        ("fg colour (OSC 10)", b"\x1b]10;?\x1b\\"),
        ("palette 1 (OSC 4)", b"\x1b]4;1;?\x1b\\"),
        ("cursor colour (OSC 12)", b"\x1b]12;?\x1b\\"),
        ("TERM query (XTVERSION)", b"\x1b[>0q"),
    ] {
        let reply = ask(&mut engine, query);
        println!("{name:24} {:?}", reply.escape_debug().to_string());
    }
}
