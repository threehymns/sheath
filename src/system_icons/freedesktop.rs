//! Looking up the desktop's icon theme: which theme is current, what its pictures are called,
//! and which file of each name to hand Rune.
//!
//! The theme named by GTK or KDE is searched first, then the themes it inherits and the
//! `hicolor` fallback, each read once into an index from icon name to the best file of that
//! name. A picture is handed to Rune's cache the first time a row asks for it and kept, so a
//! scroll costs nothing after the first frame. A name the theme does not carry is remembered
//! as missing, so a folder of text files does not rebuild the index on every frame.

use crate::app::Entry;
use rune::{Image, ImageCache};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The size an icon is rasterized at: the inspector's 52 points, a little over one device
/// pixel each, which the list and the grid scale down from.
const ICON_PIXELS: f32 = 64.0;
/// Themes keep icons a few folders deep (`128x128/apps`); a deeper tree is not an icon theme.
const MAX_DEPTH: usize = 4;

#[derive(Default)]
pub struct SystemIcons {
    cache: Option<Arc<ImageCache>>,
    themes: RefCell<Vec<Theme>>,
    scanned: Cell<bool>,
    images: RefCell<HashMap<&'static str, Option<Image>>>,
}

impl SystemIcons {
    pub fn attach(&mut self, cache: Arc<ImageCache>) {
        self.cache = Some(cache);
    }

    /// The theme's picture for a row, if the theme carries one.
    pub fn image(&self, entry: &Entry) -> Option<Image> {
        let cache = self.cache.as_ref()?;
        for &name in names(entry) {
            if let Some(kept) = self.images.borrow().get(name) {
                if kept.is_some() {
                    return kept.clone();
                }
                continue;
            }
            let image = self
                .resolve(name)
                .map(|path| cache.file(&path, ICON_PIXELS, 1.0));
            let found = image.is_some();
            self.images.borrow_mut().insert(name, image.clone());
            if found {
                return image;
            }
        }
        None
    }

    fn resolve(&self, name: &str) -> Option<PathBuf> {
        self.themes().iter().find_map(|theme| theme.find(name))
    }

    fn themes(&self) -> std::cell::Ref<'_, Vec<Theme>> {
        if !self.scanned.get() {
            self.scanned.set(true);
            *self.themes.borrow_mut() = discover();
        }
        self.themes.borrow()
    }
}

/// One theme's folder, with its icons read from disk the first time it is asked for a name.
struct Theme {
    dir: PathBuf,
    icons: RefCell<Option<HashMap<String, PathBuf>>>,
}

impl Theme {
    fn at(dir: PathBuf) -> Theme {
        Theme {
            dir,
            icons: RefCell::new(None),
        }
    }

    fn find(&self, name: &str) -> Option<PathBuf> {
        let mut icons = self.icons.borrow_mut();
        icons
            .get_or_insert_with(|| index(&self.dir))
            .get(name)
            .cloned()
    }
}

/// The icon names a row asks for, most specific first.
fn names(entry: &Entry) -> &'static [&'static str] {
    if entry.is_dir {
        return &["folder"];
    }
    let extension = entry
        .path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "avif" | "heic"
        | "heif" | "ico" | "svg" => &["image-x-generic"],
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v" | "mpg" | "mpeg" => &["video-x-generic"],
        "mp3" | "flac" | "ogg" | "wav" | "m4a" | "aac" | "opus" => &["audio-x-generic"],
        "pdf" => &["application-pdf", "x-office-document", "text-x-generic"],
        "doc" | "docx" | "odt" | "rtf" | "pages" => &["x-office-document", "text-x-generic"],
        "xls" | "xlsx" | "ods" | "csv" => &["x-office-spreadsheet", "text-x-generic"],
        "ppt" | "pptx" | "odp" | "key" => &["x-office-presentation", "text-x-generic"],
        "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "tgz" => {
            &["package-x-generic"]
        }
        "rs" | "go" | "c" | "h" | "cpp" | "hpp" | "java" | "py" | "rb" | "lua" | "js" | "ts"
        | "tsx" | "jsx" | "sh" | "bash" | "zsh" | "fish" | "toml" | "yaml" | "yml" | "json" => {
            &["text-x-script", "text-x-generic"]
        }
        _ => &["text-x-generic"],
    }
}

/// The theme folders to search, in order: the current theme, what it inherits, then `hicolor`.
fn discover() -> Vec<Theme> {
    let roots = roots();
    let mut themes = Vec::new();
    for name in chain() {
        for root in &roots {
            let dir = root.join(&name);
            if dir.is_dir() {
                themes.push(Theme::at(dir));
            }
        }
    }
    // Themes older than the icon theme spec still drop flat pictures here.
    let pixmaps = PathBuf::from("/usr/share/pixmaps");
    if pixmaps.is_dir() {
        themes.push(Theme::at(pixmaps));
    }
    themes
}

/// The current theme's name, then every theme it inherits, then `hicolor`.
fn chain() -> Vec<String> {
    let mut names = Vec::new();
    let mut queue = VecDeque::from([theme_name()]);
    while let Some(name) = queue.pop_front() {
        if names.contains(&name) {
            continue;
        }
        queue.extend(inherits(&name));
        names.push(name);
    }
    if !names.iter().any(|name| name == "hicolor") {
        names.push("hicolor".to_owned());
    }
    names
}

fn theme_name() -> String {
    gtk_theme()
        .or_else(kde_theme)
        .unwrap_or_else(|| "hicolor".to_owned())
}

fn gtk_theme() -> Option<String> {
    let config = config_home()?;
    for dir in ["gtk-4.0", "gtk-3.0"] {
        if let Ok(text) = std::fs::read_to_string(config.join(dir).join("settings.ini"))
            && let Some(value) = ini_value(&text, "gtk-icon-theme-name")
        {
            return Some(value);
        }
    }
    None
}

fn kde_theme() -> Option<String> {
    let text = std::fs::read_to_string(config_home()?.join("kdeglobals")).ok()?;
    let mut in_icons = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_icons = line == "[Icons]";
            continue;
        }
        if in_icons && let Some(value) = pair(line, "Theme") {
            return Some(value);
        }
    }
    None
}

/// The themes a theme's `index.theme` names as its parents.
fn inherits(name: &str) -> Vec<String> {
    for root in roots() {
        let index = root.join(name).join("index.theme");
        let Ok(text) = std::fs::read_to_string(&index) else {
            continue;
        };
        return ini_value(&text, "Inherits")
            .unwrap_or_default()
            .split(',')
            .map(|parent| parent.trim().to_owned())
            .filter(|parent| !parent.is_empty())
            .collect();
    }
    Vec::new()
}

fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    match std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        Some(data) => roots.push(data.join("icons")),
        None => {
            if let Some(home) = home() {
                roots.push(home.join(".local/share/icons"));
            }
        }
    }
    if let Some(home) = home() {
        roots.push(home.join(".icons"));
    }
    let dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_owned());
    for dir in dirs.split(':').filter(|dir| !dir.is_empty()) {
        roots.push(PathBuf::from(dir).join("icons"));
    }
    roots
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home().map(|home| home.join(".config")))
}

/// One `key=value` line, without its quotes and surrounding space.
fn pair(line: &str, key: &str) -> Option<String> {
    let (found, value) = line.split_once('=')?;
    if found.trim() != key {
        return None;
    }
    let value = value.trim().trim_matches('"').to_owned();
    (!value.is_empty()).then_some(value)
}

fn ini_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| pair(line, key))
}

/// Every picture a theme folder carries, by icon name, keeping the best one of each name.
fn index(dir: &Path) -> HashMap<String, PathBuf> {
    let mut icons = HashMap::new();
    walk(dir, 0, &mut icons);
    icons
}

fn walk(dir: &Path, depth: usize, icons: &mut HashMap<String, PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(&path, depth + 1, icons);
            continue;
        }
        let supported = path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("png"));
        if !supported {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        match icons.get(&name) {
            Some(kept) if rank(&path) <= rank(kept) => {}
            _ => {
                icons.insert(name, path);
            }
        }
    }
}

/// How good a file is for drawing at any size: a vector beats a raster, and among rasters the
/// one nearest 128 pixels wins, with anything under that sized down less.
fn rank(path: &Path) -> (u8, u16) {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
    {
        return (2, u16::MAX);
    }
    let size = path
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .find_map(|part| part.split('x').next()?.parse::<u16>().ok())
        .unwrap_or(0);
    let score = if size >= 128 {
        1000u16.saturating_sub((size - 128).min(900))
    } else {
        size
    };
    (1, score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_keeps_the_best_picture_for_a_name() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("Theme");
        for sub in [
            "16x16/apps",
            "128x128/apps",
            "512x512/apps",
            "scalable/apps",
        ] {
            std::fs::create_dir_all(theme.join(sub)).unwrap();
        }
        let small = theme.join("16x16/apps/folder.png");
        let near = theme.join("128x128/apps/folder.png");
        let large = theme.join("512x512/apps/folder.png");
        for path in [&small, &near, &large] {
            std::fs::write(path, b"").unwrap();
        }

        let theme = Theme::at(theme);
        assert_eq!(theme.find("folder"), Some(near), "128 wins over 16 and 512");

        let vector = dir.path().join("Theme/scalable/apps/folder.svg");
        std::fs::write(&vector, b"").unwrap();
        let theme = Theme::at(dir.path().join("Theme"));
        assert_eq!(
            theme.find("folder"),
            Some(vector),
            "a vector draws at any size, so it beats the raster"
        );
    }
}
