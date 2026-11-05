//! Image thumbnails: a setting on by default, small decodes the rows ask for, and nothing for
//! files that are not pictures or will not decode.

use super::helpers::*;
use crate::settings::{Setting, Settings};
#[cfg(not(target_arch = "wasm32"))]
use std::fs;

fn write_png(path: &std::path::Path, width: u32, height: u32) {
    use image::codecs::png::PngEncoder;
    use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};
    let image = RgbaImage::from_pixel(width, height, Rgba([30, 160, 200, 255]));
    let mut file = std::fs::File::create(path).unwrap();
    PngEncoder::new(&mut file)
        .write_image(image.as_raw(), width, height, ExtendedColorType::Rgba8)
        .unwrap();
}

fn app_with_image() -> (tempfile::TempDir, crate::app::App) {
    let dir = fixture();
    write_png(&dir.path().join("photo.png"), 64, 48);
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    (dir, app)
}

#[test]
fn thumbnails_are_on_by_default_and_name_their_setting() {
    assert!(Settings::defaults().show_thumbnails, "thumbnails start on");
    assert!(Setting::ShowThumbnails.default_on());
    assert_eq!(Setting::ShowThumbnails.name(), "show-thumbnails");
    assert_eq!(Setting::ShowThumbnails.id(), "thumbnails");
}

#[test]
fn the_thumbnail_setting_saves_to_the_config_file() {
    let dir = fixture();
    let (app, _path) = app_in_config_dir(&dir);
    let mut rt = runtime(app);
    rt.frame(0.0);

    click(&mut rt, "open-settings");
    click(&mut rt, "setting-thumbnails");
    assert!(
        !rt.state.config.show_thumbnails,
        "the switch turns thumbnails off"
    );
    click(&mut rt, "close-settings");

    let config = fs::read_to_string(dir.path().join("config.kdl")).unwrap();
    assert!(config.contains("show-thumbnails #false"), "{config}");
    assert!(!saved_settings(&dir).show_thumbnails);
}

#[test]
fn a_picture_lands_as_a_thumbnail_ready_to_draw() {
    let (_dir, mut app) = app_with_image();
    assert!(app.settle_thumbs(), "landing a thumbnail asks for a redraw");
    let entry = app
        .entries
        .iter()
        .find(|entry| entry.name == "photo.png")
        .expect("the picture is in the folder");
    assert!(entry.is_image, "the entry knows the file is a picture");
    let thumb = app
        .thumbnail(entry)
        .unwrap_or_else(|| panic!("the thumbnail landed"));
    assert!(thumb.is_ready(), "and it is ready for that frame to draw");
    assert_eq!(
        thumb.size(),
        Some([64, 48]),
        "a small picture keeps its size"
    );
}

#[test]
fn a_large_picture_decodes_at_thumbnail_size() {
    let dir = fixture();
    {
        use image::codecs::jpeg::JpegEncoder;
        use image::{ExtendedColorType, RgbImage};
        let image = RgbImage::from_pixel(2000, 1000, image::Rgb([200, 60, 30]));
        let mut file = fs::File::create(dir.path().join("photo.jpg")).unwrap();
        JpegEncoder::new(&mut file)
            .encode(image.as_raw(), 2000, 1000, ExtendedColorType::Rgb8)
            .unwrap();
    }
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    app.settle_thumbs();
    let entry = app
        .entries
        .iter()
        .find(|entry| entry.name == "photo.jpg")
        .unwrap();
    let [width, height] = app
        .thumbnail(entry)
        .expect("the thumbnail landed")
        .size()
        .unwrap();
    assert!(
        width <= 96 && height <= 96,
        "a big picture is decoded at the thumbnail size, not {width}×{height}"
    );
}

#[test]
fn a_picture_that_will_not_decode_keeps_no_thumbnail() {
    let dir = fixture();
    // A valid PNG signature with no chunks behind it: sniffed as PNG, then fails to decode.
    fs::write(dir.path().join("broken.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    app.settle_thumbs();
    let entry = app
        .entries
        .iter()
        .find(|entry| entry.name == "broken.png")
        .unwrap();
    assert!(
        entry.is_image,
        "the entry still knows it is meant to be a picture"
    );
    assert!(
        app.thumbnail(entry).is_none(),
        "and it has no thumbnail to show"
    );
}

#[test]
fn a_file_that_is_not_a_picture_asks_for_no_thumbnail() {
    let dir = fixture();
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    app.settle_thumbs();
    let entry = app
        .entries
        .iter()
        .find(|entry| entry.name == "notes.md")
        .unwrap();
    assert!(!entry.is_image, "the entry knows the file is not a picture");
    assert!(app.thumbnail(entry).is_none(), "and there is no thumbnail");
}

#[test]
fn only_the_window_pictures_are_asked_for() {
    let dir = fixture();
    for i in 0..120 {
        fs::write(dir.path().join(format!("f{i:03}.txt")), "x").unwrap();
    }
    write_png(&dir.path().join("zzz.png"), 32, 32);
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    app.rows_view = [0.0, 0.0, 800.0, 300.0];
    app.settle_thumbs();
    let entry = app
        .entries
        .iter()
        .find(|entry| entry.name == "zzz.png")
        .unwrap();
    assert!(
        app.thumbnail(entry).is_none(),
        "a picture past the window has not been decoded"
    );
    app.rows_view = [0.0, 0.0, 800.0, 100_000.0];
    app.settle_thumbs();
    let entry = app
        .entries
        .iter()
        .find(|entry| entry.name == "zzz.png")
        .map(|entry| (entry.path.clone(), entry.modified))
        .unwrap();
    assert!(
        app.thumbnail_at(&entry.0, entry.1).is_some(),
        "once it is in the window, it decodes"
    );
}

#[test]
fn the_window_keeps_its_thumbnails_past_the_offscreen_cache_limit() {
    let dir = fixture();
    for i in 0..280 {
        write_png(&dir.path().join(format!("img{i:03}.png")), 32, 32);
    }
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    app.rows_view = [0.0, 0.0, 800.0, 100_000.0];
    app.settle_thumbs();

    let missing: Vec<&str> = app
        .entries
        .iter()
        .filter(|entry| entry.is_image && app.thumbnail_at(&entry.path, entry.modified).is_none())
        .map(|entry| entry.name.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "every visible picture keeps its thumbnail: {}",
        missing.len()
    );

    // A second request pass does not trade any of them for newer decodes.
    app.settle_thumbs();
    let missing: Vec<&str> = app
        .entries
        .iter()
        .filter(|entry| entry.is_image && app.thumbnail_at(&entry.path, entry.modified).is_none())
        .map(|entry| entry.name.as_str())
        .collect();
    assert!(missing.is_empty(), "still kept: {missing:?}");
}

#[test]
fn a_thumbnail_shows_in_the_list_and_the_grid() {
    let (_dir, app) = app_with_image();
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.settle_thumbs();
    rt.frame(0.1);
    assert!(
        rt.find("thumb-f:photo.png").is_some(),
        "the list shows the thumbnail"
    );
    click(&mut rt, "toggle-layout");
    rt.state.settle_thumbs();
    rt.frame(0.1);
    assert!(
        rt.find("thumb-f:photo.png").is_some(),
        "the grid shows the thumbnail too"
    );
}

#[test]
fn turning_thumbnails_off_takes_the_picture_out_of_the_list() {
    let (_dir, app) = app_with_image();
    let mut rt = runtime(app);
    rt.frame(0.0);
    rt.state.settle_thumbs();
    rt.frame(0.1);
    assert!(rt.find("thumb-f:photo.png").is_some());

    click(&mut rt, "open-settings");
    scroll_settings_into_view(&mut rt, "setting-thumbnails");
    click(&mut rt, "setting-thumbnails");
    assert!(!rt.state.config.show_thumbnails);
    click(&mut rt, "close-settings");
    rt.frame(0.1);
    assert!(
        rt.find("thumb-f:photo.png").is_none(),
        "with thumbnails off, the row goes back to its file icon"
    );
}
