use super::helpers::*;
use crate::app::Preview;
use crate::keybinds::Action;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

fn zip_with(dir: &TempDir, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let path = dir.path().join(name);
    let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    for (inner, body) in files {
        writer
            .start_file(*inner, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap();
    path
}

fn tar_gz_with(dir: &TempDir, name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let path = dir.path().join(name);
    let file = fs::File::create(&path).unwrap();
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(gz);
    for (inner, body) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, *inner, *body).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap();
    path
}

#[test]
fn a_zip_opens_as_a_folder_and_navigates_like_one() {
    let dir = fixture();
    let archive = zip_with(
        &dir,
        "bundle.zip",
        &[
            ("readme.txt", b"hello\n"),
            ("src/main.rs", b"fn main() {}\n"),
            ("src/deep/notes.md", b"# deep\n"),
        ],
    );
    let mut app = app(&dir);
    app.open_archive(archive);

    assert!(app.in_archive());
    assert_eq!(names(&app), ["src", "readme.txt"]);

    app.open_id("f:src");
    assert_eq!(names(&app), ["deep", "main.rs"]);
    app.open_id("f:deep");
    assert_eq!(names(&app), ["notes.md"]);

    app.up();
    assert_eq!(names(&app), ["deep", "main.rs"]);
    app.up();
    assert_eq!(names(&app), ["src", "readme.txt"]);
    app.up();
    assert!(!app.in_archive(), "Up at the archive's root leaves it");
    assert_eq!(
        app.selected.as_deref(),
        Some("f:bundle.zip"),
        "the archive's own row is chosen in the folder around it"
    );
}

#[test]
fn back_and_forward_walk_into_and_out_of_an_archive() {
    let dir = fixture();
    let archive = zip_with(&dir, "bundle.zip", &[("src/main.rs", b"fn main() {}\n")]);
    let mut app = app(&dir);
    app.open_archive(archive);
    app.open_id("f:src");

    app.back();
    assert_eq!(names(&app), ["src"]);
    app.back();
    assert!(!app.in_archive(), "Back leaves the archive too");
    app.forward();
    assert!(app.in_archive(), "and Forward opens it again");
    assert_eq!(names(&app), ["src"]);
}

#[test]
fn a_tar_gz_lists_its_folders_and_previews_a_file() {
    let dir = fixture();
    let archive = tar_gz_with(
        &dir,
        "docs.tar.gz",
        &[
            ("docs/guide.md", b"# Guide\n"),
            ("docs/more/info.txt", b"more\n"),
        ],
    );
    let mut app = app(&dir);
    app.open_archive(archive);

    assert_eq!(names(&app), ["docs"]);
    app.open_id("f:docs");
    assert_eq!(names(&app), ["more", "guide.md"]);

    app.select_id("f:guide.md");
    let Some(Preview::Text(body)) = &app.preview else {
        panic!("expected a text preview from the archive");
    };
    assert!(body.contains("# Guide"));
    app.select_id("f:more");
    assert!(matches!(app.preview, Some(Preview::Folder(1))));
}

#[test]
fn a_picture_inside_an_archive_previews_from_its_bytes() {
    use image::{ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

    let picture = RgbaImage::from_pixel(64, 48, Rgba([10, 20, 30, 255]));
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(picture.as_raw(), 64, 48, ExtendedColorType::Rgba8)
        .unwrap();

    let dir = fixture();
    let archive = zip_with(&dir, "pictures.zip", &[("photo.png", &png)]);
    let mut app = app(&dir);
    app.use_images(rune::ImageCache::new(std::sync::Arc::new(|| {})));
    app.open_archive(archive);
    app.select_id("f:photo.png");

    assert!(matches!(app.preview, Some(Preview::Image)));
    app.settle_preview();
    assert!(app.preview_error.is_none(), "{:?}", app.preview_error);
    let image = app.preview_image.as_ref().expect("the picture decoded");
    assert_eq!(image.size(), Some([64, 48]));
}

#[test]
fn an_archive_is_read_only_and_its_folder_size_is_counted() {
    let dir = fixture();
    let archive = zip_with(
        &dir,
        "bundle.zip",
        &[
            ("readme.txt", b"hello\n"),
            ("src/main.rs", b"fn main() {}\n"),
        ],
    );
    let mut app = app(&dir);
    app.open_archive(archive);
    app.select_id("f:readme.txt");

    assert!(!app.action_enabled(Action::Rename));
    assert!(!app.action_enabled(Action::Delete));
    assert!(!app.action_enabled(Action::NewFolder));
    assert!(!app.action_enabled(Action::Paste));
    assert!(
        app.action_enabled(Action::CopyPath),
        "reading where a row lives is still allowed"
    );
    assert!(
        !dir.path().join("readme.txt").exists(),
        "nothing was written"
    );

    app.select_id("f:src");
    app.calculate_folder_size();
    assert_eq!(
        app.folder_size,
        Some((13, 1)),
        "the archive's own entries answer the folder's size"
    );
}

#[test]
fn the_path_bar_names_the_archive_and_the_folder_inside_it() {
    let dir = fixture();
    let archive = zip_with(&dir, "bundle.zip", &[("src/deep/notes.md", b"# deep\n")]);
    let mut app = app(&dir);
    app.open_archive(archive);
    app.open_id("f:src");

    let crumbs: Vec<String> = app.crumbs().into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        crumbs.last().map(String::as_str),
        Some("src"),
        "the folder inside the archive is the last crumb: {crumbs:?}"
    );
    assert!(
        crumbs.contains(&"bundle.zip".to_owned()),
        "the archive itself is a crumb: {crumbs:?}"
    );
    assert_eq!(
        app.location_label(),
        format!("{}/src", app.cwd.join("bundle.zip").display()),
        "the label a screen reader hears names the archive too"
    );
}
