//! Picture-preview benchmarks, ignored by default. They time the two halves of a preview: the
//! Rune image worker that reads and decodes the file (JPEG/PNG decode plus Rune's linear-light
//! mip chain), and the frames the app renders around it.
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
// The inspector draws a picture at most 420 pt square, so 840 device pixels is the size the
// decoded pixels are actually shown at.
const PREVIEW_PIXELS: f64 = (PREVIEW_POINTS * SCALE) as f64;

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

/// Time from asking the cache for a picture to the worker finishing it. `Image::wait` blocks
/// on the worker's result, so this is the decode's wall time with a warm file cache.
fn time_file(cache: &ImageCache, path: &Path) -> (f64, Image) {
    let at = Instant::now();
    let image = cache.file(path, PREVIEW_POINTS, SCALE);
    image.wait().expect("the benchmark photo decodes");
    (at.elapsed().as_secs_f64() * 1e3, image)
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

fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

fn time_median(runs: usize, mut each: impl FnMut() -> f64) -> f64 {
    median((0..runs).map(|_| each()).collect())
}

// Decode and mip cost by picture size. The point is where the milliseconds go: a full-resolution
// decode, Rune's mips, or a decode scaled to what the inspector actually draws.
#[test]
#[ignore]
fn preview_decode() {
    let dir = tempfile::tempdir().unwrap();
    let formats = [
        ("1 MP jpeg", 1152, 864, ImageFormat::Jpeg),
        ("4 MP jpeg", 2304, 1728, ImageFormat::Jpeg),
        ("12 MP jpeg", 4000, 3000, ImageFormat::Jpeg),
        ("24 MP jpeg", 6000, 4000, ImageFormat::Jpeg),
        ("12 MP png", 4000, 3000, ImageFormat::Png),
        // The size the preview is actually drawn at: what a decoder that can scale during
        // decode, a stored thumbnail, or a two-stage preview would cost.
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
        "\n{:<12} {:>7} {:>8} {:>8} {:>10} {:>11} {:>9} {:>13}",
        "photo",
        "file MB",
        "full ms",
        "mips ms",
        "decode ms",
        "decode MP/s",
        "mem MB",
        "fit-to-840ms"
    );
    for (photo, (label, _, _, format)) in photos.iter().zip(formats) {
        let full = time_median(3, || {
            let cache = ImageCache::new(no_wake());
            time_file(&cache, &photo.path).0
        });
        let mips = time_median(3, || time_mips(photo.width, photo.height));
        let decode = (full - mips).max(0.0);
        let mp = photo.megapixels();
        // Decode cost if pixels were limited to 840×630: JPEG decoders that can scale during
        // decode (or a stored thumbnail) approach this; reading the full frame then resizing
        // does not, because the decode already happened.
        let fit = decode * (PREVIEW_PIXELS * PREVIEW_PIXELS * 0.75 / 1e6) / mp;
        // Full-res RGBA plus the whole mip chain, roughly 4/3 of the top level.
        let memory = mp * 4.0 * 4.0 / 3.0;
        println!(
            "{:<12} {:>7.1} {:>8.1} {:>8.1} {:>10.1} {:>11.1} {:>9.1} {:>13.1}",
            label,
            photo.bytes as f64 / 1e6,
            full,
            mips,
            decode,
            mp / (decode / 1e3).max(f64::MIN_POSITIVE),
            memory,
            fit,
        );
        // If the cache cannot decode at the drawn size, it can still decode full size and shrink
        // before Rune's mips. This is that path, minus the 840 px mips already in the table.
        if photo.width >= 4000 && format == ImageFormat::Jpeg {
            let at = Instant::now();
            let decoded = image::load_from_memory(&std::fs::read(&photo.path).unwrap()).unwrap();
            let decode = at.elapsed().as_secs_f64() * 1e3;
            let at = Instant::now();
            let thumb = decoded.thumbnail(840, 630);
            assert_eq!(thumb.width(), 840);
            let thumbnail = at.elapsed().as_secs_f64() * 1e3;
            let at = Instant::now();
            let resized = decoded.resize(840, 630, image::imageops::FilterType::Triangle);
            let resize = at.elapsed().as_secs_f64() * 1e3;
            let scaled_mips = time_mips(resized.width(), resized.height());
            println!(
                "  {label} via downscale: decode {decode:.1} + thumbnail {thumbnail:.1} + mips(840) {scaled_mips:.1} = {:.1} ms",
                decode + thumbnail + scaled_mips
            );
            println!(
                "  {label} via resize:    decode {decode:.1} + triangle {resize:.1} + mips(840) {scaled_mips:.1} = {:.1} ms",
                decode + resize + scaled_mips
            );
        }
    }
}

// The frame the app renders when a row is selected, the wait until the worker has the picture,
// and the frame that first draws it. `frames late` converts the wait to 60 Hz frames.
#[test]
#[ignore]
fn preview_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let photos = photos(dir.path(), "nav", 4);
    let notes = dir.path().join("notes.txt");
    std::fs::write(&notes, "text preview").unwrap();

    let mut app = app(&dir);
    let dirty = Arc::new(AtomicBool::new(false));
    let flag = dirty.clone();
    app.images = Some(ImageCache::new(Arc::new(move || {
        flag.store(true, Ordering::Release);
    })));
    let mut rt = runtime(app);
    rt.frame(0.0);

    println!(
        "\n{:<30} {:>10} {:>12} {:>11} {:>12}",
        "selection", "select ms", "wait ms", "frames late", "picture ms"
    );
    let row = |rt: &mut Runtime<App>, dirty: &Arc<AtomicBool>, id: &str, label: &str| {
        let at = Instant::now();
        rt.state.select_id(id);
        rt.invalidate();
        rt.frame(1.0 / 60.0);
        let select = at.elapsed().as_secs_f64() * 1e3;

        let wait_at = Instant::now();
        loop {
            let ready = rt.state.preview_image.as_ref().is_none_or(Image::is_ready);
            if ready {
                break;
            }
            assert!(
                wait_at.elapsed() < Duration::from_secs(30),
                "{label} never decoded"
            );
            std::thread::sleep(Duration::from_millis(1));
            if dirty.swap(false, Ordering::AcqRel) {
                rt.invalidate();
            }
        }
        let wait = wait_at.elapsed().as_secs_f64() * 1e3;

        let at = Instant::now();
        rt.frame(1.0 / 60.0);
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
    row(&mut rt, &dirty, &first, "picture, first visit");
    row(&mut rt, &dirty, &second, "picture, next row");
    row(&mut rt, &dirty, &first, "picture, back again");
    row(&mut rt, &dirty, "f:notes.txt", "text file");
    row(&mut rt, &dirty, &first, "picture after text");
}

// Holding an arrow key queues one decode per row. Rune's image cache is a single worker with no
// way to supersede or cancel a queued job, unlike `Latest`, which the folder scan uses. The last
// picture therefore waits for every picture selected on the way to it.
#[test]
#[ignore]
fn preview_scrub() {
    let dir = tempfile::tempdir().unwrap();
    let photos = photos(dir.path(), "scrub", 6);
    let cache = ImageCache::new(no_wake());

    let single = time_median(3, || {
        let cache = ImageCache::new(no_wake());
        time_file(&cache, &photos[0].path).0
    });
    let at = Instant::now();
    let queued: Vec<Image> = photos
        .iter()
        .map(|photo| cache.file(&photo.path, PREVIEW_POINTS, SCALE))
        .collect();
    let last_ready = {
        let last = queued.last().unwrap();
        while !last.is_ready() {
            std::thread::sleep(Duration::from_millis(1));
        }
        at.elapsed().as_secs_f64() * 1e3
    };
    println!(
        "\n{} pictures selected in one burst, one {:.1} MP decode each:\n  single decode      {:>8.1} ms\n  last picture ready {:>8.1} ms  ({:.1}× a single decode)",
        photos.len(),
        photos[0].megapixels(),
        single,
        last_ready,
        last_ready / single.max(f64::MIN_POSITIVE),
    );
}

// Where "opening a huge folder is instant" comes from: past 2,000 entries the read runs on a
// pool worker (`Latest`, superseding), and only a window of rows is built. A preview decodes the
// whole picture on a single non-superseding worker before any of it can draw.
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
