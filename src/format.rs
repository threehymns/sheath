//! Human-facing descriptions of filesystem data: sizes, dates, kinds and permissions.

use std::fs::Metadata;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Datelike, Local};

/// A byte count with one decimal place, switching units as it grows.
pub fn size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let n = bytes as f64;
    let (value, unit) = if bytes < 1024 {
        return format!("{bytes} B");
    } else if n < KB * KB {
        (n / KB, "KB")
    } else if n < KB * KB * KB {
        (n / (KB * KB), "MB")
    } else if n < KB * KB * KB * KB {
        (n / (KB * KB * KB), "GB")
    } else {
        (n / (KB * KB * KB * KB), "TB")
    };
    let text = format!("{value:.1}");
    let text = text.trim_end_matches(".0");
    format!("{text} {unit}")
}

/// A short date for the file table: today and yesterday by name, other years omitted.
pub fn table_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "—".into();
    };
    let when: DateTime<Local> = time.into();
    let now = Local::now();
    let days = now
        .date_naive()
        .signed_duration_since(when.date_naive())
        .num_days();
    if days == 0 {
        format!("Today, {}", when.format("%H:%M"))
    } else if days == 1 {
        format!("Yesterday, {}", when.format("%H:%M"))
    } else if when.year() == now.year() {
        format!("{} {}", when.format("%b"), when.day())
    } else {
        format!("{} {}, {}", when.format("%b"), when.day(), when.year())
    }
}

/// A full date and time for the inspector.
pub fn full_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "—".into();
    };
    let when: DateTime<Local> = time.into();
    format!(
        "{} {}, {} at {}",
        when.format("%b"),
        when.day(),
        when.year(),
        when.format("%H:%M")
    )
}

/// The file's kind: a folder, an alias, or a friendlier name for a known extension.
pub fn kind(name: &str, is_dir: bool, is_symlink: bool) -> String {
    if is_dir {
        return if is_symlink {
            "Folder alias".into()
        } else {
            "Folder".into()
        };
    }
    if is_symlink {
        return "Alias".into();
    }
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let known = match ext.as_str() {
        "md" | "markdown" => "Markdown document",
        "txt" | "log" => "Text document",
        "rs" => "Rust source",
        "ts" | "tsx" => "TypeScript source",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript source",
        "py" => "Python source",
        "go" => "Go source",
        "swift" => "Swift source",
        "c" | "h" => "C source",
        "cc" | "cpp" | "hpp" => "C++ source",
        "json" => "JSON file",
        "toml" => "TOML file",
        "yaml" | "yml" => "YAML file",
        "png" => "PNG image",
        "jpg" | "jpeg" => "JPEG image",
        "gif" => "GIF image",
        "webp" => "WebP image",
        "svg" => "SVG image",
        "pdf" => "PDF document",
        "zip" => "ZIP archive",
        "tar" | "gz" | "xz" | "zst" => "Archive",
        "mp3" | "wav" | "flac" | "ogg" => "Audio",
        "mp4" | "mov" | "mkv" | "webm" => "Video",
        "sh" | "bash" | "zsh" | "fish" => "Shell script",
        "css" | "scss" => "Stylesheet",
        "html" | "htm" => "HTML document",
        _ if ext.is_empty() => "Document",
        _ => return format!("{} file", ext.to_ascii_uppercase()),
    };
    known.into()
}

/// A Unix mode such as `drwxr-xr-x`, or a plain description elsewhere.
pub fn permissions(meta: &Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        let mut out = String::with_capacity(10);
        out.push(if meta.is_dir() {
            'd'
        } else if meta.file_type().is_symlink() {
            'l'
        } else {
            '-'
        });
        for shift in [6, 3, 0] {
            let bits = (mode >> shift) & 0o7;
            out.push(if bits & 0o4 != 0 { 'r' } else { '-' });
            out.push(if bits & 0o2 != 0 { 'w' } else { '-' });
            out.push(if bits & 0o1 != 0 { 'x' } else { '-' });
        }
        format!("{out} · {:03o}", mode & 0o777)
    }
    #[cfg(not(unix))]
    {
        if meta.permissions().readonly() {
            "Read-only".into()
        } else {
            "Read & write".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_the_largest_unit_that_fits() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1024), "1 KB");
        assert_eq!(size(1536), "1.5 KB");
        assert_eq!(size(5 * 1024 * 1024), "5 MB");
        assert_eq!(size(3 * 1024 * 1024 * 1024), "3 GB");
    }

    #[test]
    fn extensions_read_as_kinds() {
        assert_eq!(kind("src", true, false), "Folder");
        assert_eq!(kind("notes.md", false, false), "Markdown document");
        assert_eq!(kind("archive.weird", false, false), "WEIRD file");
        assert_eq!(kind("LICENSE", false, false), "Document");
        assert_eq!(kind("link", false, true), "Alias");
    }
}
