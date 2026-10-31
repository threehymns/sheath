//! Picture-preview benchmarks, ignored by default. They time Rune's image cache against the
//! app's preview pipeline (decode at draw size, coalesced requests) and the frames the app
//! renders around a selection.
//!
//! Run with:
//!
//! ```sh
//! cargo test --release preview_ -- --ignored --nocapture --test-threads=1
//! ```

use super::helpers::*;
use crate::app::App;
use image::{ImageFormat, RgbImage};
use rune::{Image, ImageCache, Runtime};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const PREVIEW_POINTS: f32 = 420.0;
const SCALE: f32 = 2.0;
const PREVIEW_PIXELS: u32 = (PREVIEW_POINTS * SCALE) as u32;
const FRAME: f64 = 1.0 / 60.0;

struct Photo {
    path: PathBuf,
    name: String,
    width: u32,
    height: u32,
    bytes: u64,
}

impl Photo {
    fn megapixels(&self) -> f64 {
        self.width as f64 * self.height as f64 / 1_000_000.0
    }
}

/// A deterministic, photo-like RGB image: a two-axis gradient with a little high-frequency
/// texture, so a JPEG's entropy coding is not the degenerate one of a flat colour.
fn source(width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        let hash = (u64::from(x) * 0x9e37_79b9_7f4a_7c15) ^ (u64::from(y) * 0xc2b2_ae3d_27d4_eb4f);
        let noise = ((hash >> 40) & 0x1f) as i32 - 16;
        let low = (x * 160 / width.max(1)) as i32;
        let high = (y * 160 / height.max(1)) as i32;
        let r = (low + 30 + noise).clamp(0, 255) as u8;
        let g = (high + 40 + noise).clamp(0, 255) as u8;
        let b = (220 - (low + high) / 2 + noise).clamp(0, 255) as u8;
        image::Rgb([r, g, b])
    })
}

fn photo(dir: &Path, name: &str, width: u32, height: u32, format: ImageFormat) -> Photo {
    let source = source(width, height);
    let path = dir.join(name);
    match format {
        ImageFormat::Jpeg => {
            let file = std::fs::File::create(&path).unwrap();
            let mut writer = std::io::BufWriter::new(file);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 88)
                .encode_image(&source)
                .unwrap();
        }
        ImageFormat::Png => {
            source.save_with_format(&path, ImageFormat::Png).unwrap();
        }
        other => panic!("unsupported benchmark format {other:?}"),
    }
    Photo {
        bytes: std::fs::metadata(&path).unwrap().len(),
        path,
        name: name.to_owned(),
        width,
        height,
    }
}

fn photos(dir: &Path, prefix: &str, count: usize) -> Vec<Photo> {
    (0..count)
        .map(|i| {
            photo(
                dir,
                &format!("{prefix}{i}.jpg"),
                4000,
                3000,
                ImageFormat::Jpeg,
            )
        })
        .collect()
}

fn no_wake() -> rune::Waker {
    Arc::new(|| {})
}

/// Time from asking Rune's cache for a picture to its worker finishing it. `Image::wait` blocks
/// on the worker's result, so this is the decode's wall time with a warm file cache.
fn time_file(cache: &ImageCache, path: &Path) -> f64 {
    let at = Instant::now();
    let image = cache.file(path, PREVIEW_POINTS, SCALE);
    image.wait().expect("the benchmark photo decodes");
    at.elapsed().as_secs_f64() * 1e3
}

/// The same work `ImageCache::file` does after its decoder returns: validate the RGBA, then
/// premultiply and build the full mip chain down to 1×1. `rgba` also hashes its input on the
/// calling thread before queueing the work, and `file` hashes only a short path, so that hash is
/// timed out to keep the two comparable.
fn time_mips(width: u32, height: u32) -> f64 {
    let mut bytes = vec![0u8; width as usize * height as usize * 4];
    for (i, pixel) in bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let value = (i % 251) as u8;
        *pixel = [value, value / 2, 255 - value, 255];
    }
    let cache = ImageCache::new(no_wake());
    let at = Instant::now();
    let image = cache.rgba(width, height, bytes);
    let hashed = at.elapsed();
    image.wait().expect("the mip fixture decodes");
    (at.elapsed() - hashed).as_secs_f64() * 1e3
}

/// What the app spends from file to a drawable preview: its own decode at preview size plus
/// Rune's mips for that image, or Rune's whole cache when the picture is small enough to hand
/// over untouched.
fn time_pipeline(path: &Path) -> f64 {
    let at = Instant::now();
    match crate::app::preview::bench_decode(path, PREVIEW_PIXELS) {
        Ok((width, height, _)) => at.elapsed().as_secs_f64() * 1e3 + time_mips(width, height),
        Err(_) => time_file(&ImageCache::new(no_wake()), path),
    }
}

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn time_median(runs: usize, mut each: impl FnMut() -> f64) -> f64 {
    median((0..runs).map(|_| each()).collect())
}

// Where the milliseconds go, Rune's cache against the app's decoder. Rune decodes the full
// picture and builds its mips from that; the app decodes no larger than the pane draws and only
// asks Rune for the mips of that small image.
#[test]
#[ignore]
fn preview_decode() {
    let dir = tempfile::tempdir().unwrap();
    let formats = [
        ("1 MP jpeg", 1152, 864, ImageFormat::Jpeg),
        ("4 MP jpeg", 2304, 1728, ImageFormat::Jpeg),
        ("12 MP jpeg", 4000, 3000, ImageFormat::Jpeg),
        ("24 MP jpeg", 6000, 4000, ImageFormat::Jpeg),
        ("1 MP png", 1152, 864, ImageFormat::Png),
        ("12 MP png", 4000, 3000, ImageFormat::Png),
        ("840 px jpeg", 840, 630, ImageFormat::Jpeg),
        ("320 px jpeg", 320, 240, ImageFormat::Jpeg),
    ];
    let photos: Vec<Photo> = formats
        .iter()
        .map(|(name, width, height, format)| {
            photo(dir.path(), &format!("{name}.img"), *width, *height, *format)
        })
        .collect();

    println!(
        "\n{:<12} {:>7} {:>9} {:>8} {:>8} {:>10} {:>9}",
        "photo", "file MB", "Rune ms", "decode", "mips", "sheath ms", "speedup"
    );
    for (photo, (label, _, _, _)) in photos.iter().zip(formats) {
        let cache = ImageCache::new(no_wake());
        let rune = time_median(3, || time_file(&cache, &photo.path));
        let mips = time_mips(photo.width, photo.height);
        let sheath = time_median(3, || time_pipeline(&photo.path));
        println!(
            "{:<12} {:>7.1} {:>9.1} {:>8.1} {:>8.1} {:>10.1} {:>8.1}×",
            label,
            photo.bytes as f64 / 1e6,
            rune,
            (rune - mips).max(0.0),
            mips,
            sheath,
            rune / sheath.max(f64::MIN_POSITIVE),
        );
    }
    println!(
        "decode = Rune ms minus separately timed mips. sheath = the app's decode at preview\n\
         size plus Rune's mips for it, or Rune's whole cache when it hands a small picture over.\n\
         A 12 MP preview holds ~64 MB decoded + mips as Rune builds it, and ~2.8 MB at 840 px."
    );
}

// The frame the app renders when a row is selected, the wait until the app has a picture to
// draw, and the frame that first draws it. `frames late` converts the wait to 60 Hz frames.
#[test]
#[ignore]
fn preview_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let photos = photos(dir.path(), "nav", 4);
    std::fs::write(dir.path().join("notes.txt"), "text preview").unwrap();

    let (mut rt, dirty) = preview_runtime(&dir);
    println!(
        "\n{:<30} {:>10} {:>12} {:>11} {:>12}",
        "selection", "select ms", "wait ms", "frames late", "picture ms"
    );
    let row = |rt: &mut Runtime<App>, id: &str, label: &str| {
        let at = Instant::now();
        rt.state.select_id(id);
        rt.invalidate();
        rt.frame(FRAME);
        let select = at.elapsed().as_secs_f64() * 1e3;

        let wait_at = Instant::now();
        loop {
            let ready = picture_ready(rt);
            if ready {
                break;
            }
            assert!(
                wait_at.elapsed() < Duration::from_secs(30),
                "{label} never decoded"
            );
            std::thread::sleep(Duration::from_millis(1));
            // The window would redraw on the worker's wake; that frame is what lands the
            // finished decode.
            if dirty.swap(false, Ordering::AcqRel) {
                rt.invalidate();
                rt.frame(FRAME);
            }
        }
        let wait = wait_at.elapsed().as_secs_f64() * 1e3;

        let at = Instant::now();
        rt.frame(FRAME);
        let picture = at.elapsed().as_secs_f64() * 1e3;
        println!(
            "{:<30} {:>10.2} {:>12.1} {:>11.1} {:>12.2}",
            label,
            select,
            wait,
            wait / (1000.0 / 60.0),
            picture
        );
    };

    let first = format!("f:{}", photos[0].name);
    let second = format!("f:{}", photos[1].name);
    row(&mut rt, &first, "picture, first visit");
    row(&mut rt, &second, "picture, next row");
    row(&mut rt, &first, "picture, back again");
    row(&mut rt, "f:notes.txt", "text file");
    row(&mut rt, &first, "picture after text");
}

// Holding an arrow key selects faster than a picture decodes. Rune's cache queues every one;
// the app debounces the burst and decodes only the row it stops on.
#[test]
#[ignore]
fn preview_scrub() {
    let dir = tempfile::tempdir().unwrap();
    let photos = photos(dir.path(), "scrub", 6);
    let cache = ImageCache::new(no_wake());

    let single = time_median(3, || time_file(&cache, &photos[0].path));
    let at = Instant::now();
    let queued: Vec<Image> = photos
        .iter()
        .map(|photo| cache.file(&photo.path, PREVIEW_POINTS, SCALE))
        .collect();
    while !queued.last().unwrap().is_ready() {
        std::thread::sleep(Duration::from_millis(1));
    }
    let rune = at.elapsed().as_secs_f64() * 1e3;

    let (mut rt, dirty) = preview_runtime(&dir);
    let start = Instant::now();
    for photo in &photos {
        rt.state.select_id(&format!("f:{}", photo.name));
        rt.invalidate();
        rt.frame(FRAME);
        std::thread::sleep(Duration::from_millis(16));
    }
    let last_at = Instant::now();
    loop {
        if picture_ready(&rt) {
            break;
        }
        assert!(
            last_at.elapsed() < Duration::from_secs(30),
            "scrub never settled"
        );
        std::thread::sleep(Duration::from_millis(1));
        if dirty.swap(false, Ordering::AcqRel) {
            rt.invalidate();
            rt.frame(FRAME);
        }
    }
    let app = start.elapsed().as_secs_f64() * 1e3;
    let last = last_at.elapsed().as_secs_f64() * 1e3;
    println!(
        "\n{} pictures selected, one {:.1} MP decode each:\n  Rune cache queued    {:>7.1} ms ({:.1}× single)\n  app, held arrow key  {:>7.1} ms ({:.1}× single), last picture {:.1} ms after the final key",
        photos.len(),
        photos[0].megapixels(),
        rune,
        rune / single,
        app,
        app / single,
        last,
    );
}

// Where "opening a huge folder is instant" comes from: past 2,000 entries the read runs on a
// pool worker (`Latest`, superseding), and only a window of rows is built.
#[test]
#[ignore]
fn preview_folder_open() {
    println!(
        "\n{:<10} {:>12} {:>14} {:>12}",
        "entries", "open ms", "scan done ms", "first frame ms"
    );
    for count in [200usize, 2_000, 20_000] {
        let dir = long_folder(count);
        let at = Instant::now();
        let mut app = App::open_with(
            dir.path().to_path_buf(),
            crate::settings::Settings::defaults(),
        );
        let open = at.elapsed().as_secs_f64() * 1e3;

        let scan_at = Instant::now();
        app.settle_scan();
        let scan = scan_at.elapsed().as_secs_f64() * 1e3;

        let mut rt = runtime(app);
        let at = Instant::now();
        rt.frame(0.0);
        let frame = at.elapsed().as_secs_f64() * 1e3;
        println!("{count:<10} {open:>12.2} {scan:>14.1} {frame:>12.2}");
    }
}

/// Whether the selection's preview has settled: a ready picture, a decode error, or a row that
/// is not a picture at all.
fn picture_ready(rt: &Runtime<App>) -> bool {
    rt.state.preview_error.is_some()
        || !matches!(rt.state.preview, Some(crate::app::Preview::Image))
        || rt.state.preview_image.as_ref().is_some_and(Image::is_ready)
}

/// A runtime whose app has the real preview waker, so a finished decode asks for a frame.
fn preview_runtime(dir: &tempfile::TempDir) -> (Runtime<App>, Arc<AtomicBool>) {
    let mut app = app(dir);
    let dirty = Arc::new(AtomicBool::new(false));
    let flag = dirty.clone();
    let waker: rune::Waker = Arc::new(move || {
        flag.store(true, Ordering::Release);
    });
    crate::app::setup(&mut app, waker);
    let mut rt = runtime(app);
    rt.frame(0.0);
    (rt, dirty)
}
