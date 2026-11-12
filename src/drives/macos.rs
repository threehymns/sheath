//! macOS drives through `/Volumes` and `diskutil`.
//!
//! A mounted volume is only a drive when `diskutil info` says it is a local, non-virtual
//! external disk. That keeps internal partitions, mounted disk images and network shares out of
//! the sidebar, and gives the row the disk identifier the unmount command wants.
//!
//! An external disk the user has unmounted no longer has an entry under `/Volumes`, so
//! `diskutil list external` finds it and `diskutil info` says which entries are volumes rather
//! than EFI partitions or APFS snapshots.

use super::Drive;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn probe() -> Result<Vec<Drive>, String> {
    let mut drives = mounted();
    for (device, name) in unmounted() {
        if drives.iter().any(|drive| drive.device == device) {
            continue;
        }
        drives.push(Drive {
            id: format!("volume:{device}"),
            name,
            device,
            mountpoint: None,
        });
    }
    Ok(drives)
}

pub(super) fn mount(drive: &Drive) -> Result<Option<PathBuf>, String> {
    let output = Command::new("diskutil")
        .arg("mount")
        .arg(&drive.device)
        .output()
        .map_err(|err| format!("Could not mount “{}”: {err}", drive.name))?;
    if !output.status.success() {
        return Err(format!(
            "Could not mount “{}”: {}",
            drive.name,
            detail(&output)
        ));
    }
    // `diskutil mount` does not reliably print where the volume landed, so ask for the mount
    // point here while the work is still off the UI thread. The next probe can confirm the list.
    let Some(info) = info(Path::new(&drive.device)) else {
        return Err(format!(
            "Mounted “{}” but could not read its folder.",
            drive.name
        ));
    };
    let path = field(&info, "Mount Point")
        .filter(|path| path.starts_with('/'))
        .map(PathBuf::from)
        .ok_or_else(|| format!("Mounted “{}” but could not find its folder.", drive.name))?;
    Ok(Some(path))
}

pub(super) fn unmount(drive: &Drive) -> Result<(), String> {
    // `diskutil unmount` takes either the mount point or the device node; prefer the folder so
    // the volume it names is the one the sidebar showed.
    let target = drive
        .path()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(&drive.device));
    let output = Command::new("diskutil")
        .arg("unmount")
        .arg(&target)
        .output()
        .map_err(|err| format!("Could not unmount “{}”: {err}", drive.name))?;
    if !output.status.success() {
        return Err(format!(
            "Could not unmount “{}”: {}",
            drive.name,
            detail(&output)
        ));
    }
    Ok(())
}

fn mounted() -> Vec<Drive> {
    let Ok(entries) = std::fs::read_dir("/Volumes") else {
        return Vec::new();
    };
    let boot = std::fs::canonicalize("/").ok();
    let mut drives = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() || !path.is_dir() {
            continue;
        }
        let is_boot = std::fs::canonicalize(&path)
            .ok()
            .zip(boot.clone())
            .is_some_and(|(volume, boot)| volume == boot);
        if is_boot {
            continue;
        }
        let Some(fallback_name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        // `.timemachine` and the like are bookkeeping folders, not disks a user mounts.
        if fallback_name.starts_with('.') {
            continue;
        }
        let Some(info) = info(&path) else {
            // Network shares do not answer `diskutil info`; they are not local drives.
            continue;
        };
        if field(&info, "Internal").is_some_and(|internal| internal.eq_ignore_ascii_case("yes")) {
            continue;
        }
        // A mounted disk image is ejectable, but it is not an external disk.
        if field(&info, "Virtual")
            .is_some_and(|virtual_disk| virtual_disk.eq_ignore_ascii_case("yes"))
        {
            continue;
        }
        if field(&info, "Mounted").is_some_and(|mounted| mounted.eq_ignore_ascii_case("no")) {
            continue;
        }
        let Some(device) = field(&info, "Device Identifier").filter(|device| !device.is_empty())
        else {
            continue;
        };
        let name = field(&info, "Volume Name")
            .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("not applicable"))
            .unwrap_or(fallback_name);
        drives.push(Drive {
            id: format!("volume:{device}"),
            name,
            device,
            mountpoint: Some(path),
        });
    }
    drives
}

/// External volumes that are not mounted. They have no entry in `/Volumes`, so their names
/// come from `diskutil info`.
fn unmounted() -> Vec<(String, String)> {
    let Ok(output) = Command::new("diskutil").args(["list", "external"]).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut identifiers = BTreeSet::new();
    for line in text.lines() {
        if !line.starts_with(' ') {
            continue;
        }
        let Some(identifier) = line.split_whitespace().last() else {
            continue;
        };
        let Some(rest) = identifier.strip_prefix("disk") else {
            continue;
        };
        // A volume is `diskN sM`: one `s`. The whole disk, an EFI partition and an APFS
        // snapshot (`diskN sMsN`) are not places to mount.
        if rest.is_empty() || rest.matches('s').count() != 1 {
            continue;
        }
        if rest.chars().all(|ch| ch.is_ascii_digit() || ch == 's') {
            identifiers.insert(identifier.to_owned());
        }
    }
    let mut found = Vec::new();
    for identifier in identifiers {
        let Some(info) = info(Path::new(&identifier)) else {
            continue;
        };
        if field(&info, "Mounted").is_some_and(|mounted| mounted.eq_ignore_ascii_case("yes")) {
            continue;
        }
        if is_efi(&info) {
            continue;
        }
        let Some(name) = field(&info, "Volume Name")
            .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("not applicable"))
        else {
            continue;
        };
        // A container or partition table has a name but no filesystem to browse.
        if field(&info, "File System Personality").is_none_or(|kind| kind.is_empty()) {
            continue;
        }
        found.push((identifier, name));
    }
    found
}

fn is_efi(info: &str) -> bool {
    ["Partition Type", "Content (IOContent)"]
        .iter()
        .any(|key| field(info, key).is_some_and(|kind| kind.eq_ignore_ascii_case("efi")))
}

fn info(target: &Path) -> Option<String> {
    let output = Command::new("diskutil")
        .arg("info")
        .arg(target)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn field(info: &str, key: &str) -> Option<String> {
    info.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim().to_owned())
    })
}

fn detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if text.is_empty() {
        "the command reported an error".to_owned()
    } else {
        text.to_owned()
    }
}
