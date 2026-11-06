//! The native decoders behind a preview: decode no larger than the inspector draws.
//!
//! A preview asks for at most `key.pixels` on a side. JPEG shrinks by DCT scale, which costs a
//! fraction of a full decode; PNG, BMP, GIF, WebP, TIFF, ICO, AVIF and HEIC have no scaled decode
//! in the crates this app uses, so they decode whole and shrink before Rune builds its mips.
//! SVG, and anything Rune's cache reads better, is handed to Rune instead.

use super::{Decoded, Key, apk};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage};
use std::borrow::Cow;
use std::io::Cursor;

const MAX_SIDE: u32 = 8_192;
const MAX_ALLOC: u64 = 256 * 1024 * 1024;

pub(super) fn decode(key: &Key) -> Decoded {
    let extension = key
        .path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    // A file inside an archive has no path Rune's cache could read, so even an SVG is decoded
    // from the bytes that came with the key.
    if matches!(extension.as_str(), "svg" | "svgz") && key.bytes.is_none() {
        return Decoded::ViaRune(key.path.clone());
    }
    let bytes: Cow<'_, [u8]> = match &key.bytes {
        Some(bytes) => Cow::Borrowed(&bytes[..]),
        None => match crate::vfs::read(&key.path) {
            Ok(bytes) => Cow::Owned(bytes),
            Err(err) => return Decoded::Failed(err.to_string()),
        },
    };
    if key.bytes.is_some() {
        return embedded(&bytes, &extension, key);
    }
    let bytes = bytes.as_ref();
    // An APK is a ZIP archive whose face is its launcher icon, not a raster to sniff.
    if extension == "apk" {
        return apk::icon(bytes, key.pixels);
    }
    // AVIF and HEIC are ISOBMFF rasters whose codecs the image crate does not carry.
    match extension.as_str() {
        "avif" => return avif(bytes, key),
        "heic" | "heif" => return heic(bytes, key),
        _ => {}
    }
    match image::guess_format(bytes) {
        Ok(ImageFormat::Jpeg) => jpeg(bytes, key),
        // Rune decodes PNG itself, so a small one is still handed over; the formats below it
        // cannot read, so those are decoded here whatever their size.
        Ok(ImageFormat::Png) => raster(bytes, key, true),
        Ok(format) if extra_raster(format) => raster(bytes, key, false),
        // SVG (sniffed by Rune) and anything else Rune's cache either decodes or reports in
        // the inspector as before.
        _ => Decoded::ViaRune(key.path.clone()),
    }
}

/// A file whose bytes came with the key, as an archive's do: there is no path on disk to hand
/// to Rune, so every raster is decoded here and an SVG goes to Rune as bytes.
fn embedded(bytes: &[u8], extension: &str, key: &Key) -> Decoded {
    if extension == "svg" {
        return Decoded::Svg(bytes.to_vec());
    }
    if extension == "apk" {
        return apk::icon(bytes, key.pixels);
    }
    match extension {
        "avif" => return avif(bytes, key),
        "heic" | "heif" => return heic(bytes, key),
        _ => {}
    }
    match image::guess_format(bytes) {
        Ok(ImageFormat::Jpeg | ImageFormat::Png) => raster(bytes, key, false),
        Ok(format) if extra_raster(format) => raster(bytes, key, false),
        _ => Decoded::Failed("This file has no preview.".into()),
    }
}

/// AVIF also has no scaled decode: decode whole, then shrink. The decoder applies the
/// container's own transforms, so no EXIF orientation is read here.
fn avif(bytes: &[u8], key: &Key) -> Decoded {
    let image = avif_decode::Decoder::from_avif(bytes)
        .and_then(|decoder| decoder.to_image())
        .map_err(|err| err.to_string());
    match image {
        Ok(image) => {
            let (width, height, pixels) = avif_rgba(image);
            match shrink(width, height, pixels, key.pixels) {
                Ok((width, height, pixels)) => Decoded::Rgba {
                    width,
                    height,
                    pixels,
                },
                Err(err) => Decoded::Failed(err),
            }
        }
        Err(err) => Decoded::Failed(err),
    }
}

/// AVIF hands back whatever layout the file carries; a preview is always RGBA.
fn avif_rgba(image: avif_decode::Image) -> (u32, u32, Vec<u8>) {
    use avif_decode::Image;
    match image {
        Image::Rgba8(image) => (
            image.width() as u32,
            image.height() as u32,
            image
                .buf()
                .iter()
                .flat_map(|p| [p.r, p.g, p.b, p.a])
                .collect(),
        ),
        Image::Rgb8(image) => (
            image.width() as u32,
            image.height() as u32,
            image
                .buf()
                .iter()
                .flat_map(|p| [p.r, p.g, p.b, 255])
                .collect(),
        ),
        Image::Rgba16(image) => (
            image.width() as u32,
            image.height() as u32,
            image
                .buf()
                .iter()
                .flat_map(|p| {
                    [
                        (p.r >> 8) as u8,
                        (p.g >> 8) as u8,
                        (p.b >> 8) as u8,
                        (p.a >> 8) as u8,
                    ]
                })
                .collect(),
        ),
        Image::Rgb16(image) => (
            image.width() as u32,
            image.height() as u32,
            image
                .buf()
                .iter()
                .flat_map(|p| [(p.r >> 8) as u8, (p.g >> 8) as u8, (p.b >> 8) as u8, 255])
                .collect(),
        ),
        Image::Gray8(image) => (
            image.width() as u32,
            image.height() as u32,
            image
                .buf()
                .iter()
                .flat_map(|p| [p.value(), p.value(), p.value(), 255])
                .collect(),
        ),
        Image::Gray16(image) => (
            image.width() as u32,
            image.height() as u32,
            image
                .buf()
                .iter()
                .flat_map(|p| {
                    let v = (p.value() >> 8) as u8;
                    [v, v, v, 255]
                })
                .collect(),
        ),
    }
}

/// HEIC/HEIF, decoded whole too. The decoder applies the container's orientation and converts
/// Display P3 to sRGB, so the result is drawn as it arrives.
fn heic(bytes: &[u8], key: &Key) -> Decoded {
    match heif_oxide::decode_bytes(bytes) {
        Ok(image) => {
            let (width, height) = (image.width, image.height);
            match shrink(width, height, image.to_rgba8(), key.pixels) {
                Ok((width, height, pixels)) => Decoded::Rgba {
                    width,
                    height,
                    pixels,
                },
                Err(err) => Decoded::Failed(err),
            }
        }
        Err(err) => Decoded::Failed(err.to_string()),
    }
}

/// The common rasters this app decodes for itself because Rune's cache cannot read them.
fn extra_raster(format: ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Bmp
            | ImageFormat::Gif
            | ImageFormat::WebP
            | ImageFormat::Tiff
            | ImageFormat::Ico
    )
}

/// Decode a JPEG with the smallest DCT scale that still covers the preview, so a large photo
/// never becomes a full-resolution RGBA buffer. A picture too small for a half-size decode is
/// handed to Rune: decoding whole is faster in its decoder, and needs no rewrite.
fn jpeg(bytes: &[u8], key: &Key) -> Decoded {
    match scaled_jpeg(bytes, key.pixels) {
        Ok(Some((width, height, pixels))) => Decoded::Rgba {
            width,
            height,
            pixels,
        },
        Ok(None) => Decoded::ViaRune(key.path.clone()),
        Err(err) => Decoded::Failed(err),
    }
}

fn scaled_jpeg(bytes: &[u8], max: u32) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
    use jpeg_decoder::PixelFormat;
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.set_max_decoding_buffer_size(MAX_ALLOC as usize);
    decoder.read_info().map_err(|err| err.to_string())?;
    let info = decoder.info().ok_or("no JPEG header")?;
    if !matches!(info.pixel_format, PixelFormat::RGB24 | PixelFormat::L8) {
        return Err(format!(
            "unsupported JPEG pixel format {:?}",
            info.pixel_format
        ));
    }
    let format = info.pixel_format;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    // A DCT scale can shrink by 1/2, 1/4 or 1/8. Below twice the preview there is no scale that
    // still covers it, and decoding whole is slower here than in Rune's decoder, so hand it over.
    if width.max(height) < max.saturating_mul(2) {
        return Ok(None);
    }
    let orientation = orientation_of(bytes);
    let (target_width, target_height) = fit(width, height, max);
    decoder
        .scale(target_width as u16, target_height as u16)
        .map_err(|err| err.to_string())?;
    let decoded = decoder.decode().map_err(|err| err.to_string())?;
    let info = decoder.info().ok_or("no JPEG header")?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    let pixels = match format {
        PixelFormat::L8 => decoded.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        _ => decoded
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    };
    finish(width, height, pixels, orientation, max).map(Some)
}

/// PNG, and the other rasters, have no scaled decode: inflate, then shrink before Rune builds
/// its mips. A PNG too small to gain from the resize goes to Rune untouched. An animated GIF or
/// WebP previews as its first frame, a TIFF as its first page, and an ICO as the size its
/// encoder picked.
fn raster(bytes: &[u8], key: &Key, hand_off_small: bool) -> Decoded {
    match scaled_raster(bytes, key.pixels, hand_off_small) {
        Ok(Some((width, height, pixels))) => Decoded::Rgba {
            width,
            height,
            pixels,
        },
        Ok(None) => Decoded::ViaRune(key.path.clone()),
        Err(err) => Decoded::Failed(err),
    }
}

fn scaled_raster(
    bytes: &[u8],
    max: u32,
    hand_off_small: bool,
) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder().map_err(|err| err.to_string())?;
    let (width, height) = decoder.dimensions();
    // Under twice the preview a resize saves little; Rune's own path is at least as fast.
    if hand_off_small && width.max(height) < max.saturating_mul(2) {
        return Ok(None);
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    limits
        .reserve(decoder.total_bytes())
        .map_err(|err| err.to_string())?;
    decoder.set_limits(limits).map_err(|err| err.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|err| err.to_string())?;
    image.apply_orientation(orientation);
    let (width, height) = (image.width(), image.height());
    shrink(width, height, image.into_rgba8().into_raw(), max).map(Some)
}

/// Shrink a decoded picture to the preview box, box-filtered, and hand back its RGBA.
pub(super) fn shrink(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    max: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let image = RgbaImage::from_raw(width, height, pixels)
        .ok_or("decoded pixels do not match the image size")?;
    let image = DynamicImage::ImageRgba8(image);
    let image = if image.width() > max || image.height() > max {
        image.thumbnail(max, max)
    } else {
        image
    };
    let image = image.into_rgba8();
    Ok((image.width(), image.height(), image.into_raw()))
}

fn finish(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    orientation: Orientation,
    max: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let image = RgbaImage::from_raw(width, height, pixels)
        .ok_or("decoded pixels do not match the image size")?;
    let mut image = DynamicImage::ImageRgba8(image);
    image.apply_orientation(orientation);
    if image.width() > max || image.height() > max {
        image = image.resize(max, max, image::imageops::FilterType::Triangle);
    }
    let image = image.into_rgba8();
    Ok((image.width(), image.height(), image.into_raw()))
}

/// The biggest box with the picture's aspect ratio that fits in `max` square.
fn fit(width: u32, height: u32, max: u32) -> (u32, u32) {
    let scale = (max as f64 / width.max(height).max(1) as f64).min(1.0);
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

/// EXIF orientation from the container's header only; the pixels still come from the scaled
/// JPEG decode, which does not read EXIF itself.
fn orientation_of(bytes: &[u8]) -> Orientation {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_decoder().ok())
        .and_then(|mut decoder| decoder.orientation().ok())
        .unwrap_or(Orientation::NoTransforms)
}
