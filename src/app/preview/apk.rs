//! An APK's own icon, read out of the package for a thumbnail.
//!
//! An APK is a ZIP archive. The launcher icon cannot be found by file name: resource shrinking
//! renames `res/mipmap-*/ic_launcher.png` to something like `res/ab.png`, and modern apps ship an
//! adaptive icon, a pair of vector drawables. So the manifest's `android:icon` resource id is
//! resolved through `resources.arsc` to whichever file it names: a raster is decoded, and a
//! vector or adaptive icon is rewritten as SVG for Rune to rasterize. A package whose manifest
//! or table will not parse falls back to picking a drawable by its entry name.

mod axml;
mod table;
mod vector;

use super::Decoded;
use super::decode::shrink;
use std::io::{Cursor, Read};
use table::Value as Resource;

enum Icon {
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    Svg(Vec<u8>),
}

pub(super) fn icon(bytes: &[u8], max: u32) -> Decoded {
    match extract(bytes, max) {
        Ok(Icon::Rgba {
            width,
            height,
            pixels,
        }) => Decoded::Rgba {
            width,
            height,
            pixels,
        },
        Ok(Icon::Svg(svg)) => Decoded::Svg(svg),
        Err(err) => Decoded::Failed(err),
    }
}

fn extract(bytes: &[u8], max: u32) -> Result<Icon, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|err| err.to_string())?;
    match named(&mut archive, max) {
        Ok(icon) => Ok(icon),
        Err(error) => listed(&mut archive, max).map_err(|_| error),
    }
}

/// The icon the manifest names, through the resource table. This is the path that survives
/// resource shrinking and adaptive icons.
fn named(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, max: u32) -> Result<Icon, String> {
    let manifest = read(archive, "AndroidManifest.xml").ok_or("the APK has no manifest")?;
    let manifest = axml::parse(&manifest).ok_or("the APK's manifest will not parse")?;
    let application = manifest
        .root
        .child("application")
        .ok_or("the APK has no application")?;
    let id = ["icon", "roundIcon"]
        .iter()
        .find_map(|name| application.attr(name).and_then(axml::Value::reference))
        .ok_or("the application names no icon")?;
    let table = read(archive, "resources.arsc").ok_or("the APK has no resource table")?;
    let table = table::parse(&table).ok_or("the APK's resource table will not parse")?;
    let path = match table.resolve(id) {
        Some(Resource::Path(path)) => path,
        _ => return Err("the icon resource names no file".into()),
    };
    let picture = read(archive, &path).ok_or("the APK is missing its icon file")?;
    if path.to_ascii_lowercase().ends_with(".xml") {
        return drawing(archive, &table, &picture);
    }
    raster(&picture, max)
}

/// An adaptive icon is a background and a foreground layer, each a drawable reference. Draw the
/// layers that are vectors on one canvas in order; a bitmap layer is left out.
fn drawing(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    table: &table::Table,
    bytes: &[u8],
) -> Result<Icon, String> {
    let document = axml::parse(bytes).ok_or("the APK's icon will not parse")?;
    match document.root.name.as_str() {
        "vector" => Ok(Icon::Svg(
            vector::vector(&document.root, table)?.into_bytes(),
        )),
        "adaptive-icon" => {
            let mut layers = Vec::new();
            for name in ["background", "foreground"] {
                let Some(layer) = document.root.child(name) else {
                    continue;
                };
                let Some(id) = layer.attr("drawable").and_then(axml::Value::reference) else {
                    continue;
                };
                let Some(Resource::Path(path)) = table.resolve(id) else {
                    continue;
                };
                if !path.to_ascii_lowercase().ends_with(".xml") {
                    continue;
                }
                let Some(bytes) = read(archive, &path) else {
                    continue;
                };
                let Some(layer) = axml::parse(&bytes) else {
                    continue;
                };
                layers.push(layer.root);
            }
            if layers.is_empty() {
                return Err("the adaptive icon has no vector layers".into());
            }
            let layers: Vec<&axml::Node> = layers.iter().collect();
            Ok(Icon::Svg(vector::adaptive(&layers, table)?.into_bytes()))
        }
        _ => Err("the icon resource is not a drawing".into()),
    }
}

/// The old shape: a package with no readable table, or a table id that names nothing, is
/// searched by entry name. This is what ordinary, unshrunk Android packages carry.
fn listed(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, max: u32) -> Result<Icon, String> {
    let mut best: Option<(Rank, usize)> = None;
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let Ok(name) = entry.name() else {
            continue;
        };
        let Some(rank) = rank(&name) else {
            continue;
        };
        if best.is_none_or(|(kept, _)| rank > kept) {
            best = Some((rank, index));
        }
    }
    let (_, index) = best.ok_or("the APK carries no launcher icon")?;
    let mut entry = archive.by_index(index).map_err(|err| err.to_string())?;
    let mut picture = Vec::new();
    entry
        .read_to_end(&mut picture)
        .map_err(|err| err.to_string())?;
    raster(&picture, max)
}

fn raster(bytes: &[u8], max: u32) -> Result<Icon, String> {
    let picture = image::load_from_memory(bytes)
        .map_err(|err| format!("the APK's icon will not decode: {err}"))?
        .into_rgba8();
    let (width, height, pixels) =
        shrink(picture.width(), picture.height(), picture.into_raw(), max)?;
    Ok(Icon::Rgba {
        width,
        height,
        pixels,
    })
}

fn read(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// How good a named package entry is as the app's icon: a name that says it is the launcher
/// icon, then a `mipmap` folder over a `drawable` one, then the densest density bucket.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    name: u8,
    mipmap: bool,
    dpi: u32,
}

fn rank(name: &str) -> Option<Rank> {
    let lower = name.to_ascii_lowercase();
    if !lower.starts_with("res/") {
        return None;
    }
    let file = lower.rsplit('/').next()?;
    // Rasters only: an adaptive icon is XML, and a nine-patch's stretch markers are not pixels.
    let stem = file
        .strip_suffix(".png")
        .or_else(|| file.strip_suffix(".webp"))?;
    if stem.ends_with(".9") || stem.ends_with("_background") {
        return None;
    }
    let mipmap = lower.split('/').any(|part| part.starts_with("mipmap"));
    let name = if matches!(
        stem,
        "ic_launcher" | "ic_launcher_round" | "icon" | "app_icon"
    ) {
        3
    } else if stem.starts_with("ic_launcher") {
        2
    } else if stem.contains("icon") || stem.contains("launcher") {
        1
    } else if mipmap {
        0
    } else {
        // A drawable that does not name itself is a picture the app ships, not its face.
        return None;
    };
    Some(Rank {
        name,
        mipmap,
        dpi: density(&lower),
    })
}

/// The density bucket in an entry's folder. Longer names come first: `xxhdpi` contains `xhdpi`.
fn density(name: &str) -> u32 {
    const BUCKETS: [(&str, u32); 6] = [
        ("xxxhdpi", 640),
        ("xxhdpi", 480),
        ("xhdpi", 320),
        ("hdpi", 240),
        ("mdpi", 160),
        ("ldpi", 120),
    ];
    BUCKETS
        .iter()
        .find_map(|(tag, dpi)| name.contains(tag).then_some(*dpi))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_densest_launcher_icon_wins_and_other_resources_do_not() {
        let small = rank("res/mipmap-mdpi/ic_launcher.png").unwrap();
        let large = rank("res/mipmap-xxxhdpi-v4/ic_launcher.png").unwrap();
        assert!(large > small, "the densest launcher icon wins");
        assert!(
            rank("res/mipmap-hdpi/app_logo.png").is_some(),
            "an unnamed mipmap is a last resort"
        );
        assert!(rank("res/mipmap-anydpi-v26/ic_launcher.xml").is_none());
        assert!(rank("res/mipmap-hdpi/ic_launcher_background.png").is_none());
        assert!(rank("res/drawable-hdpi/splash.png").is_none());
        assert!(rank("res/drawable-hdpi/panel.9.png").is_none());
        assert!(rank("assets/photo.png").is_none());
    }
}
