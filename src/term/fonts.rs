//! The characters the terminal's own face cannot draw: an installed Nerd Font, as a fallback.
//!
//! Prompt glyphs (a branch, a folder, a language mark) are private-use characters only a patched
//! font draws, and whoever put one on their system did it for their shell. Nothing is bundled:
//! the face is found in the platform's font directories, then named after the theme's mono in the
//! family list, so the shaper reaches for it only for a character the mono has no glyph for.
//! Ordinary text keeps the theme's face, and a machine without a Nerd Font is unchanged.
//!
//! Finding it means walking font directories and asking fontconfig, which can take long enough to
//! be the first thing the user waits on. The walk therefore runs on a thread of its own while the
//! window opens and the terminal starts, and the glyph atlas picks the face up when it lands.

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
use std::sync::atomic::{AtomicU64, Ordering};

/// A font file the pane can draw with: the family Rune knows it by, and its bytes. Both are
/// leaked so Rune's text system and the glyph atlas can hold them for the life of the process.
struct Face {
    family: &'static str,
    bytes: &'static [u8],
}

#[cfg(not(target_arch = "wasm32"))]
static FACE: OnceLock<Option<Face>> = OnceLock::new();

/// Zero until the walk finishes, one after: the glyph atlas rebuilds with the fallback when this
/// moves, so a face found after the first frame still reaches the grid. Only the raster build
/// has an atlas to rebuild; the others resolve before launch or have no system fonts.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
static RESOLVED: AtomicU64 = AtomicU64::new(0);

/// What a window launches with: the font files to register (the theme's, plus the Nerd Font when
/// it is already known) and the sans and mono family lists. Every window, the app and the
/// standalone terminal alike, opens from this one spec, so the fallback is wired the same way.
///
/// This does not wait for the walk. The default renderer rasterizes the grid itself, so the face
/// can arrive after launch and the atlas picks it up; the text renderer shapes rows with Rune,
/// which only takes a family at launch, so that build resolves first.
pub(crate) fn spec() -> (&'static [&'static [u8]], (&'static str, &'static str)) {
    #[cfg(feature = "text-renderer")]
    {
        let _ = face();
    }
    #[cfg(not(feature = "text-renderer"))]
    start();
    (fonts(), families())
}

/// Looks for the Nerd Font on a thread of its own, once per process: a few thousand directory
/// entries and an `fc-list` are not something the first frame should wait behind. A thread that
/// cannot be spawned falls back to resolving here, slow walk and all, so the feature still works.
/// The text renderer resolves before launch instead, so it never starts this.
#[cfg(not(feature = "text-renderer"))]
pub(crate) fn start() {
    #[cfg(target_arch = "wasm32")]
    {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static STARTED: OnceLock<()> = OnceLock::new();
        STARTED.get_or_init(|| {
            let worker = std::thread::Builder::new()
                .name("nerd-fonts".into())
                .spawn(|| {
                    let _ = face();
                    RESOLVED.store(1, Ordering::Release);
                });
            if worker.is_err() {
                let _ = face();
                RESOLVED.store(1, Ordering::Release);
            }
        });
    }
}

/// Moves when the walk has finished, whether or not it found a face; a cache keyed on it rebuilds
/// once, then stays. Only the raster cache keys on it: the other builds cannot take a family
/// after launch, so they never look.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
pub(crate) fn generation() -> u64 {
    RESOLVED.load(Ordering::Acquire)
}

fn families() -> (&'static str, &'static str) {
    static FAMILIES: OnceLock<(&'static str, &'static str)> = OnceLock::new();
    *FAMILIES.get_or_init(|| {
        let (sans, mono) = rune_theme_opencode::FAMILIES;
        match face_if_ready() {
            Some(face) => (
                sans,
                Box::leak(with_fallback(mono, face.family).into_boxed_str()),
            ),
            None => (sans, mono),
        }
    })
}

/// `mono` with `family` after it in the CSS list: the shaper falls through to it only for the
/// characters `mono` does not draw, so a Nerd Font can supply its icons without changing the
/// text around them.
fn with_fallback(mono: &str, family: &str) -> String {
    format!("{mono}, \"{family}\"")
}

/// The font files the app registers with Rune: the theme's own, plus the Nerd Font when it was
/// found before launch, so its family can be named by the mono slot. Leaked once, as
/// `AppSpec::fonts` holds them for the process.
fn fonts() -> &'static [&'static [u8]] {
    static FONTS: OnceLock<&'static [&'static [u8]]> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut list: Vec<&'static [u8]> = crate::FONTS.to_vec();
        if let Some(face) = face_if_ready() {
            list.push(face.bytes);
        }
        Box::leak(list.into_boxed_slice())
    })
}

/// The Nerd Font's bytes for the glyph atlas, which fills the cells the text path never shapes.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
pub(crate) fn fallback_face() -> Option<&'static [u8]> {
    face_if_ready().map(|face| face.bytes)
}

/// The face when the walk has already found one. Never waits: launch asks this while the worker
/// may still be walking, and must get the bundled fonts rather than stall on it.
#[cfg(not(target_arch = "wasm32"))]
fn face_if_ready() -> Option<&'static Face> {
    FACE.get().and_then(Option::as_ref)
}

#[cfg(not(target_arch = "wasm32"))]
fn face() -> Option<&'static Face> {
    FACE.get_or_init(find).as_ref()
}

#[cfg(target_arch = "wasm32")]
fn face_if_ready() -> Option<&'static Face> {
    None
}

#[cfg(all(target_arch = "wasm32", feature = "text-renderer"))]
fn face() -> Option<&'static Face> {
    None
}

/// Reads the first candidate that is a Nerd Font. A file that fails to parse is skipped rather
/// than ending the search: font directories hold plenty that is not a font.
#[cfg(not(target_arch = "wasm32"))]
fn find() -> Option<Face> {
    let mut candidates: Vec<(u8, PathBuf)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in font_files() {
        let Some(name) = file_name(&path) else {
            continue;
        };
        let Some(rank) = rank(&name) else {
            continue;
        };
        if seen.insert(path.clone()) {
            candidates.push((rank, path));
        }
    }
    // The monospaced face first, then any Nerd Font, each by path so a machine with several
    // installed always picks the same one.
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (_, path) in candidates {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let Some(font) = swash::FontRef::from_index(bytes, 0) else {
            continue;
        };
        let Some(family) = font
            .localized_strings()
            .find(|string| string.id() == swash::StringId::Family)
            .map(|string| string.to_string())
        else {
            continue;
        };
        if !family.to_lowercase().contains("nerd") {
            continue;
        }
        return Some(Face {
            family: Box::leak(family.into_boxed_str()),
            bytes,
        });
    }
    None
}

/// How much the pane wants a file, from the name it was installed under: `None` for a face it
/// would not draw with. Nerd Fonts ship one file per weight and style, and a terminal wants the
/// regular monospaced one: the `Mono` builds keep every icon on a single cell, where the plain
/// builds pair a narrow icon with a wide one.
#[cfg(not(target_arch = "wasm32"))]
fn rank(name: &str) -> Option<u8> {
    if !name.contains("nerd") || name.contains("symbols") || name.contains("propo") {
        return None;
    }
    let styled = [
        "bold",
        "italic",
        "oblique",
        "light",
        "thin",
        "medium",
        "black",
        "semibold",
        "extrabold",
        "retina",
    ];
    if styled.iter().any(|style| name.contains(style)) {
        return None;
    }
    Some(if name.contains("nerdfontmono") {
        2
    } else if name.contains("nerdfont") {
        1
    } else {
        0
    })
}

/// The platform's font directories, the ones a user installs into and the ones the system ships.
#[cfg(not(target_arch = "wasm32"))]
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Library/Fonts"));
        }
        dirs.push("/Library/Fonts".into());
        dirs.push("/System/Library/Fonts".into());
    } else if cfg!(windows) {
        let root =
            std::env::var_os("WINDIR").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
        dirs.push(root.join("Fonts"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
        }
    } else {
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".local/share/fonts"));
            dirs.push(home.join(".fonts"));
        }
        let data =
            std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
        dirs.extend(
            data.split(':')
                .filter(|dir| !dir.is_empty())
                .map(|dir| Path::new(dir).join("fonts")),
        );
    }
    dirs
}

/// Every font file under the platform's directories, plus the ones fontconfig knows about
/// outside them (a Nix store path, say).
#[cfg(not(target_arch = "wasm32"))]
fn font_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack = font_dirs();
    while let Some(dir) = stack.pop() {
        if visited.len() > 4096 || !visited.insert(dir.clone()) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if file_name(&path).is_some() {
                files.push(path);
            }
        }
    }
    #[cfg(all(unix, not(target_vendor = "apple"), not(target_os = "android")))]
    if let Ok(out) = std::process::Command::new("fc-list")
        .arg("--format=%{file}\n")
        .output()
    {
        files.extend(
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(PathBuf::from)
                .filter(|path| file_name(path).is_some()),
        );
    }
    files
}

/// `path`'s file name, lowercased without spaces or dots, for a file a font can be: the way a
/// Nerd Font install names its faces.
#[cfg(not(target_arch = "wasm32"))]
fn file_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    [".ttf", ".otf", ".ttc", ".otc"]
        .iter()
        .any(|ext| name.ends_with(ext))
        .then(|| name.replace([' ', '.'], ""))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{rank, with_fallback};

    #[test]
    fn only_a_regular_nerd_face_is_a_candidate() {
        // The monospaced build first: its icons all fit one cell.
        assert_eq!(rank("jetbrainsmononerdfontmono-regular.ttf"), Some(2));
        assert_eq!(rank("caskaydiacovenerdfont-regular.otf"), Some(1));
        // A weight or slant is not the face the grid draws with.
        assert_eq!(rank("firacodenerdfont-bold.ttf"), None);
        assert_eq!(rank("hacknerdfont-italic.ttf"), None);
        // Icons alone cover none of the text, and a proportional face has no cell.
        assert_eq!(rank("symbolsnerdfont-regular.ttf"), None);
        assert_eq!(rank("jetbrainsmononerdfontpropo-regular.ttf"), None);
        // Anything that is not a Nerd Font is not this module's business.
        assert_eq!(rank("inter-regular.ttf"), None);
    }

    #[test]
    fn the_fallback_follows_the_mono_so_only_missing_glyphs_reach_it() {
        assert_eq!(
            with_fallback("IBM Plex Mono", "JetBrainsMono Nerd Font Mono"),
            "IBM Plex Mono, \"JetBrainsMono Nerd Font Mono\""
        );
    }
}
