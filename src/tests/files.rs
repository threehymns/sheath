use super::helpers::*;
use crate::keybinds::Action;
use rune::Tx;
use std::fs;

#[test]
fn new_folder_picks_a_free_name_and_selects_it() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();

    app.new_folder(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New Folder").is_dir());
    assert_eq!(app.selected.as_deref(), Some("f:New Folder"));

    app.new_folder(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New Folder 2").is_dir());
    assert_eq!(app.selected.as_deref(), Some("f:New Folder 2"));
    assert!(names(&app).contains(&"New Folder 2"));
}

#[test]
fn the_toolbar_button_creates_and_selects_a_folder() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    let (x, y) = rt
        .find("new-folder")
        .expect("the New Folder button is in the toolbar");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.1);
    rt.frame(0.1);
    assert!(dir.path().join("New Folder").is_dir());
    assert_eq!(rt.state.selected.as_deref(), Some("f:New Folder"));
}

#[test]
fn the_inspector_builds_a_scrollable_preview() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:notes.md");
    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(rt.find("preview-scroll").is_some());
    assert!(rt.find("files-table").is_some());
}

#[test]
fn the_inspector_swaps_details_without_moving_them() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    assert!(
        rt.presented("inspector")
            .is_some_and(|(_, opacity)| opacity == 1.0),
        "the pane starts on its empty state"
    );

    let (x, y) = rt.find("f:notes.md").expect("the row is on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);

    let mut swapped = 0;
    for e in rt.elements() {
        if e.exiting || !e.path.contains("pane-body/inspector") {
            continue;
        }
        swapped += 1;
        assert_eq!(e.opacity, 1.0, "{} fades in", e.path);
        assert!(!e.moving, "{} glides", e.path);
    }
    assert!(swapped > 3, "the details are drawn: {swapped} elements");
}

#[test]
fn the_inspector_summarises_a_selection_of_several_rows() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);
    click(&mut rt, "f:file2.txt");
    click_with(&mut rt, "f:notes.md", shift());

    assert!(rt.find("selection-summary").is_some());
    assert!(
        rt.find("preview-scroll").is_none(),
        "no single file preview for several rows"
    );
    assert_eq!(rt.state.selection_size(), 3 + 4 + 24);
    assert_eq!(rt.state.selection_folders(), 0);
}

#[test]
fn copy_and_paste_puts_a_copy_beside_the_file() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:notes.md");

    app.run_action(Action::Copy, &mut tx);
    assert_eq!(
        fs::read_to_string(dir.path().join("notes.md")).unwrap(),
        "# Notes\n\nHello preview.\n",
        "copying leaves the file alone"
    );
    app.paste();
    app.settle_scan();

    assert_eq!(
        fs::read_to_string(dir.path().join("notes copy.md")).unwrap(),
        "# Notes\n\nHello preview.\n"
    );
    assert_eq!(
        app.selected.as_deref(),
        Some("f:notes copy.md"),
        "the copy is selected"
    );

    app.paste();
    assert!(
        dir.path().join("notes copy 2.md").exists(),
        "a second paste numbers itself"
    );
}

#[test]
fn cut_and_paste_moves_the_file_into_the_folder_on_screen() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:file2.txt");
    app.run_action(Action::Cut, &mut tx);
    assert!(
        app.is_cut(&dir.path().join("file2.txt")),
        "cut rows are dimmed"
    );

    app.navigate(dir.path().join("alpha"));
    app.paste();
    app.settle_scan();

    assert!(dir.path().join("alpha").join("file2.txt").exists());
    assert!(!dir.path().join("file2.txt").exists());
    assert!(app.clipboard.is_none(), "a cut is pasted once");
    assert_eq!(app.selected.as_deref(), Some("f:file2.txt"));
}

#[test]
fn duplicate_copies_a_folder_and_everything_in_it() {
    let dir = fixture();
    let mut app = app(&dir);
    fs::create_dir(dir.path().join("alpha").join("inner")).unwrap();
    fs::write(dir.path().join("alpha").join("inner").join("a.txt"), "a").unwrap();
    let mut tx = Tx::default();

    app.select_id("f:alpha");
    app.run_action(Action::Duplicate, &mut tx);

    assert!(dir.path().join("alpha copy").is_dir());
    assert_eq!(
        fs::read_to_string(dir.path().join("alpha copy").join("inner").join("a.txt")).unwrap(),
        "a",
        "the folder's contents came with it"
    );
    assert!(dir.path().join("alpha").is_dir());
}

#[test]
fn copying_a_selection_copies_every_row() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:file2.txt");
    app.selection.insert("f:file10.txt".into());

    app.run_action(Action::Copy, &mut tx);
    app.paste();
    app.settle_scan();

    assert!(dir.path().join("file2 copy.txt").exists());
    assert!(dir.path().join("file10 copy.txt").exists());
    assert_eq!(app.selection_len(), 2, "both copies are selected");
}

#[test]
fn there_is_no_paste_until_something_is_copied() {
    let dir = fixture();
    let mut app = app(&dir);
    assert!(!app.action_enabled(Action::Paste));
    app.select_id("f:file2.txt");
    app.copy_selection(false);
    assert!(app.action_enabled(Action::Paste));

    app.paste();
    assert!(dir.path().join("file2 copy.txt").exists());
}

#[test]
fn paste_drops_into_the_folder_that_is_selected() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();
    app.select_id("f:notes.md");
    app.run_action(Action::Copy, &mut tx);

    app.select_id("f:alpha");
    app.paste();
    app.settle_scan();

    assert!(
        dir.path().join("alpha").join("notes copy.md").exists(),
        "the copy lands inside the chosen folder"
    );
    assert!(!dir.path().join("notes copy.md").exists());
}

#[test]
fn new_file_picks_a_free_name_and_selects_it() {
    let dir = fixture();
    let mut app = app(&dir);
    let mut tx = Tx::default();

    app.new_file(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New File.txt").is_file());
    assert_eq!(app.selected.as_deref(), Some("f:New File.txt"));
    assert_eq!(
        fs::read_to_string(dir.path().join("New File.txt")).unwrap(),
        ""
    );

    app.new_file(&mut tx);
    app.settle_scan();
    assert!(dir.path().join("New File 2.txt").is_file());
    assert_eq!(app.selected.as_deref(), Some("f:New File 2.txt"));
}

#[test]
fn folders_first_can_be_turned_off() {
    let dir = fixture();
    fs::create_dir(dir.path().join("zeta")).unwrap();
    let mut app = app(&dir);
    assert_eq!(
        names(&app).last(),
        Some(&"notes.md"),
        "folders sort above files"
    );

    app.toggle_folders_first();
    assert!(!app.config.folders_first);
    assert_eq!(
        names(&app).last(),
        Some(&"zeta"),
        "files come first when the setting is off"
    );
}

#[test]
fn a_folder_size_is_read_on_demand_and_forgotten_with_the_selection() {
    let dir = fixture();
    fs::create_dir(dir.path().join("alpha").join("inner")).unwrap();
    fs::write(dir.path().join("alpha").join("inner").join("a.txt"), "abc").unwrap();
    let mut app = app(&dir);

    app.select_id("f:alpha");
    assert!(
        app.folder_size.is_none(),
        "a folder's size is not read up front"
    );
    app.calculate_folder_size();
    assert!(
        app.folder_size.is_none(),
        "the press starts a walk on a worker instead of doing it in the frame"
    );
    assert!(app.settle_size(), "the finished walk lands");
    assert_eq!(
        app.folder_size,
        Some((3, 2)),
        "the button counts the file below and the folder around it"
    );

    app.select_id("f:notes.md");
    assert!(
        app.folder_size.is_none(),
        "choosing another row drops the old answer"
    );
}

#[test]
fn a_folder_size_that_lands_after_the_selection_moved_is_dropped() {
    let dir = fixture();
    fs::write(dir.path().join("alpha").join("a.txt"), "abc").unwrap();
    let mut app = app(&dir);

    app.select_id("f:alpha");
    app.calculate_folder_size();
    app.select_id("f:notes.md");
    assert!(!app.settle_size(), "a stale walk is not taken");
    assert!(
        app.folder_size.is_none(),
        "another row never shows the size of the folder before it"
    );
}

#[test]
fn a_picture_previews_as_a_picture_and_not_as_its_bytes() {
    let dir = fixture();
    fs::write(dir.path().join("picture.png"), [0x89, b'P', b'N', b'G']).unwrap();
    fs::write(dir.path().join("drawing.svg"), "<svg/>").unwrap();
    let mut app = app(&dir);

    app.select_id("f:picture.png");
    assert!(
        matches!(app.preview, Some(crate::app::Preview::Image)),
        "a PNG is drawn rather than read as text"
    );
    app.select_id("f:drawing.svg");
    assert!(
        matches!(app.preview, Some(crate::app::Preview::Image)),
        "so is a vector drawing"
    );
    app.select_id("f:notes.md");
    assert!(
        matches!(app.preview, Some(crate::app::Preview::Text(_))),
        "text still previews as text"
    );
}

#[test]
fn a_picture_previews_responsively_in_the_inspector() {
    let dir = fixture();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="200" height="100"/></svg>"#;
    fs::write(dir.path().join("drawing.svg"), svg).unwrap();
    let mut app = app(&dir);
    let (wake_tx, _) = std::sync::mpsc::channel();
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(move || {
        let _ = wake_tx.send(());
    })));
    app.select_id("f:drawing.svg");
    app.settle_preview();

    let mut rt = runtime(app);
    rt.frame(0.0);
    assert!(rt.find("preview-stage").is_some(), "preview stage exists");
    let image_el = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("preview-image"))
        .expect("preview image exists");
    assert!(image_el.rect[2] > 0.0 && image_el.rect[3] > 0.0);
    // Aspect ratio of 200/100 = 2.0
    let ratio = image_el.rect[2] / image_el.rect[3];
    assert!((ratio - 2.0).abs() < 0.05, "aspect ratio is kept: {ratio}");

    let initial_w = image_el.rect[2];

    // Widen inspector pane:
    rt.state.inspector.set_width(400.0);
    rt.frame(1.0);
    let widened_el = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("preview-image"))
        .unwrap();
    assert!(
        widened_el.rect[2] > initial_w,
        "image expands with wider inspector: {} -> {}",
        initial_w,
        widened_el.rect[2]
    );
}

// A window only redraws when poll lands something, so a preview must not land pending: the
// worker waits for Rune's mips, and the one frame that lands the picture can draw it.
#[test]
fn a_picture_lands_ready_to_draw() {
    let dir = fixture();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="200" height="100"/></svg>"#;
    fs::write(dir.path().join("drawing.svg"), svg).unwrap();
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    app.select_id("f:drawing.svg");

    assert!(
        app.settle_preview(),
        "landing the picture asks for a redraw"
    );
    let image = app.preview_image.as_ref().expect("the picture landed");
    assert!(image.is_ready(), "and it is ready for that frame to draw");
    assert!(
        !app.land_preview(),
        "a ready picture doesn't keep asking for frames"
    );
}

#[test]
fn a_picture_shows_a_decoding_state_and_handles_decoding_errors() {
    let dir = fixture();
    // A valid PNG signature with no chunks behind it: sniffed as PNG, then fails to decode.
    fs::write(dir.path().join("broken.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let mut app = app(&dir);
    let (wake_tx, _) = std::sync::mpsc::channel();
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(move || {
        let _ = wake_tx.send(());
    })));
    app.select_id("f:broken.png");

    let mut rt = runtime(app);
    rt.frame(0.0);

    // After worker processes the broken image, an error message is shown:
    rt.state.settle_preview();
    assert!(
        rt.state.preview_error.is_some(),
        "a broken picture reports why it could not decode"
    );
    rt.frame(1.0);
    let error_text = rt.elements().into_iter().find(|e| {
        e.text
            .as_deref()
            .is_some_and(|t| t.contains("Could not decode picture"))
    });
    assert!(
        error_text.is_some(),
        "error note is shown for invalid picture"
    );
}

#[test]
fn the_other_common_picture_formats_decode_in_the_preview() {
    use image::codecs::bmp::BmpEncoder;
    use image::codecs::gif::GifEncoder;
    use image::codecs::ico::IcoEncoder;
    use image::codecs::tiff::TiffEncoder;
    use image::codecs::webp::WebPEncoder;
    use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

    let dir = fixture();
    let file = |name: &str| std::fs::File::create(dir.path().join(name)).unwrap();
    let small = RgbaImage::from_pixel(64, 48, Rgba([200, 60, 30, 255]));
    let (w, h, raw) = (small.width(), small.height(), small.as_raw());
    let mut bmp = file("picture.bmp");
    BmpEncoder::new(&mut bmp)
        .encode(raw, w, h, ExtendedColorType::Rgba8)
        .unwrap();
    GifEncoder::new(file("picture.gif"))
        .encode(raw, w, h, ExtendedColorType::Rgba8)
        .unwrap();
    WebPEncoder::new_lossless(file("picture.webp"))
        .encode(raw, w, h, ExtendedColorType::Rgba8)
        .unwrap();
    TiffEncoder::new(file("picture.tiff"))
        .write_image(raw, w, h, ExtendedColorType::Rgba8)
        .unwrap();
    let mut ico = file("picture.ico");
    IcoEncoder::new(&mut ico)
        .write_image(raw, w, h, ExtendedColorType::Rgba8)
        .unwrap();
    // A picture far bigger than the pane: the preview must shrink it rather than hand the whole
    // thing to Rune's mip chain.
    let large = RgbaImage::from_pixel(2000, 1000, Rgba([30, 160, 200, 255]));
    let mut large_bmp = file("large.bmp");
    BmpEncoder::new(&mut large_bmp)
        .encode(large.as_raw(), 2000, 1000, ExtendedColorType::Rgba8)
        .unwrap();

    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    for name in [
        "picture.bmp",
        "picture.gif",
        "picture.webp",
        "picture.tiff",
        "picture.ico",
    ] {
        app.select_id(&format!("f:{name}"));
        assert!(
            matches!(app.preview, Some(crate::app::Preview::Image)),
            "{name} previews as a picture"
        );
        app.settle_preview();
        assert!(
            app.preview_error.is_none(),
            "{name}: {:?}",
            app.preview_error
        );
        let image = app
            .preview_image
            .as_ref()
            .unwrap_or_else(|| panic!("{name} decoded"));
        assert_eq!(image.size(), Some([w, h]), "{name} keeps its size");
    }

    app.select_id("f:large.bmp");
    app.settle_preview();
    assert!(app.preview_error.is_none(), "{:?}", app.preview_error);
    let image = app
        .preview_image
        .as_ref()
        .expect("the large picture decoded");
    let [width, height] = image.size().expect("the large picture has a size");
    assert!(
        width <= 840 && height <= 840,
        "a big picture is decoded at the preview size, not {width}×{height}"
    );
}

// Tiny fixtures made with heif-enc and ffmpeg's AVIF muxer: a 64×48 still each.
#[test]
fn avif_and_heic_previews_decode() {
    let dir = fixture();
    fs::write(
        dir.path().join("photo.avif"),
        include_bytes!("fixtures/picture.avif"),
    )
    .unwrap();
    fs::write(
        dir.path().join("photo.heic"),
        include_bytes!("fixtures/picture.heic"),
    )
    .unwrap();
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    for name in ["photo.avif", "photo.heic"] {
        app.select_id(&format!("f:{name}"));
        assert!(
            matches!(app.preview, Some(crate::app::Preview::Image)),
            "{name} previews as a picture"
        );
        app.settle_preview();
        assert!(
            app.preview_error.is_none(),
            "{name}: {:?}",
            app.preview_error
        );
        let image = app
            .preview_image
            .as_ref()
            .unwrap_or_else(|| panic!("{name} decoded"));
        assert_eq!(
            image.size(),
            Some([64, 48]),
            "{name} decoded at its own size"
        );
    }
}

#[cfg(unix)]
#[test]
fn making_an_alias_points_a_new_name_at_the_row() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:file2.txt");
    app.make_aliases();
    app.settle_scan();

    let alias = dir.path().join("file2 alias.txt");
    assert!(alias.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&alias).unwrap(), "two");
    assert_eq!(
        app.selected.as_deref(),
        Some("f:file2 alias.txt"),
        "the alias is chosen"
    );

    app.select_id("f:file2.txt");
    app.make_aliases();
    assert!(
        dir.path().join("file2 alias 2.txt").exists(),
        "a second alias numbers itself"
    );
}

#[cfg(unix)]
#[test]
fn an_alias_of_a_folder_opens_as_a_folder() {
    let dir = fixture();
    let mut app = app(&dir);
    app.select_id("f:alpha");
    app.make_aliases();
    app.settle_scan();

    let alias = dir.path().join("alpha alias");
    assert!(alias.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(alias.is_dir());
}
