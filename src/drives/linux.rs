//! Linux drives through `lsblk` and `udisksctl`.
//!
//! A drive counts as external when the kernel marks it removable or when it arrived over USB.
//! That keeps the boot disk, internal SATA disks and the loop devices out of the sidebar while
//! showing USB sticks, card readers and external enclosures, mounted or not.

use super::Drive;
use std::path::PathBuf;
use std::process::Command;

pub(super) fn probe() -> Result<Vec<Drive>, String> {
    let output = Command::new("lsblk")
        .args(["-P", "-o", "NAME,LABEL,UUID,MOUNTPOINT,RM,TRAN,TYPE,FSTYPE"])
        .output()
        .map_err(|err| format!("Could not list drives: {err}"))?;
    if !output.status.success() {
        return Err(format!("Could not list drives: {}", detail(&output)));
    }
    Ok(parse(&String::from_utf8_lossy(&output.stdout)))
}

pub(super) fn mount(drive: &Drive) -> Result<Option<PathBuf>, String> {
    let output = Command::new("udisksctl")
        .args(["mount", "-b", &drive.device])
        .output()
        .map_err(|err| {
            format!(
                "Could not mount “{}”: {err}. udisks2 is needed to mount drives from here.",
                drive.name
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "Could not mount “{}”: {}",
            drive.name,
            detail(&output)
        ));
    }
    Ok(mount_point(&String::from_utf8_lossy(&output.stdout)))
}

pub(super) fn unmount(drive: &Drive) -> Result<(), String> {
    let output = Command::new("udisksctl")
        .args(["unmount", "-b", &drive.device])
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

/// `udisksctl mount` ends with `Mounted /dev/sdb1 at /run/media/me/Backup.`; a locale that
/// translates it leaves the refresh in `Drives` to find the folder instead.
fn mount_point(text: &str) -> Option<PathBuf> {
    let (_, tail) = text.rsplit_once(" at ")?;
    let path = tail.trim().trim_end_matches('.').trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn parse(text: &str) -> Vec<Drive> {
    let mut drives: Vec<Drive> = Vec::new();
    for line in text.lines() {
        let fields = fields(line);
        let get = |key: &str| {
            fields
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.as_str())
                .unwrap_or("")
        };
        let kind = get("TYPE");
        if !matches!(kind, "part" | "disk") || get("FSTYPE").is_empty() {
            continue;
        }
        // An internal SATA disk can report itself hot-pluggable; its transport is what tells it
        // apart from a drive in an external enclosure.
        let external = get("RM") == "1" || get("TRAN") == "usb";
        let name = get("NAME");
        if !external || name.is_empty() {
            continue;
        }
        let mountpoint = get("MOUNTPOINT");
        // A hot-pluggable root disk is still the machine's own disk, not a place to eject.
        if mountpoint == "/" {
            continue;
        }
        let uuid = get("UUID");
        let id = if uuid.is_empty() {
            format!("volume:{name}")
        } else {
            format!("volume:{uuid}")
        };
        if drives.iter().any(|drive| drive.id == id) {
            continue;
        }
        let label = get("LABEL");
        drives.push(Drive {
            id,
            name: if label.is_empty() {
                name.to_owned()
            } else {
                label.to_owned()
            },
            device: format!("/dev/{name}"),
            mountpoint: (!mountpoint.is_empty()).then(|| PathBuf::from(mountpoint)),
        });
    }
    drives
}

/// The `KEY="value"` pairs `lsblk -P` writes on one line.
fn fields(line: &str) -> Vec<(&str, String)> {
    let mut fields = Vec::new();
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start();
        let Some((key, after_key)) = trimmed.split_once('=') else {
            break;
        };
        let Some(after_quote) = after_key.strip_prefix('"') else {
            break;
        };
        let mut value = String::new();
        let mut ended = None;
        let mut chars = after_quote.char_indices();
        while let Some((at, ch)) = chars.next() {
            match ch {
                '"' => {
                    ended = Some(at + 1);
                    break;
                }
                '\\' => {
                    let Some((_, escaped)) = chars.next() else {
                        break;
                    };
                    if escaped == 'x' {
                        // `lsblk` writes a space in a label or mount point as `\x20`.
                        let high = chars.next().and_then(|(_, ch)| ch.to_digit(16));
                        let low = chars.next().and_then(|(_, ch)| ch.to_digit(16));
                        if let (Some(high), Some(low)) = (high, low)
                            && let Some(decoded) = char::from_u32(high * 16 + low)
                        {
                            value.push(decoded);
                        }
                    } else {
                        value.push(match escaped {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                    }
                }
                _ => value.push(ch),
            }
        }
        let Some(end) = ended else {
            break;
        };
        fields.push((key, value));
        rest = &after_quote[end..];
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unmounted_stick_is_listed_beside_a_mounted_one() {
        let text = concat!(
            "NAME=\"loop0\" LABEL=\"\" UUID=\"\" MOUNTPOINT=\"/snap/core\" RM=\"0\" TRAN=\"\" TYPE=\"loop\" FSTYPE=\"squashfs\"\n",
            "NAME=\"sda\" LABEL=\"\" UUID=\"root\" MOUNTPOINT=\"/\" RM=\"0\" TRAN=\"sata\" TYPE=\"disk\" FSTYPE=\"ext4\"\n",
            "NAME=\"sdb1\" LABEL=\"Backup\" UUID=\"1A2B-3C4D\" MOUNTPOINT=\"/run/media/me/Backup\" RM=\"1\" TRAN=\"usb\" TYPE=\"part\" FSTYPE=\"exfat\"\n",
            "NAME=\"sdc1\" LABEL=\"Stick\" UUID=\"\" MOUNTPOINT=\"\" RM=\"0\" TRAN=\"usb\" TYPE=\"part\" FSTYPE=\"vfat\"\n",
        );
        let drives = parse(text);
        assert_eq!(drives.len(), 2, "only the two removable drives: {drives:?}");
        assert_eq!(drives[0].id, "volume:1A2B-3C4D");
        assert_eq!(drives[0].name, "Backup");
        assert_eq!(drives[0].device, "/dev/sdb1");
        assert_eq!(
            drives[0].path(),
            Some(std::path::Path::new("/run/media/me/Backup"))
        );
        assert_eq!(drives[1].id, "volume:sdc1");
        assert_eq!(drives[1].name, "Stick");
        assert!(!drives[1].is_mounted(), "the stick waits to be mounted");
    }

    #[test]
    fn a_mount_point_with_a_space_keeps_it() {
        let text = "NAME=\"sdb1\" LABEL=\"My\\x20Disk\" UUID=\"\" MOUNTPOINT=\"/run/media/me/My\\x20Disk\" RM=\"1\" TRAN=\"usb\" TYPE=\"part\" FSTYPE=\"exfat\"\n";
        let drives = parse(text);
        assert_eq!(drives[0].name, "My Disk");
        assert_eq!(
            drives[0].path(),
            Some(std::path::Path::new("/run/media/me/My Disk"))
        );
    }

    #[test]
    fn a_mount_point_is_read_from_udisks_output() {
        assert_eq!(
            mount_point("Mounted /dev/sdb1 at /run/media/me/Backup.\n"),
            Some(PathBuf::from("/run/media/me/Backup"))
        );
        assert_eq!(mount_point("something else\n"), None);
    }
}
