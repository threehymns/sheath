//! Windows removable drives through PowerShell.
//!
//! Windows gives a removable volume its drive letter when it is plugged in, so the list is the
//! mounted volumes themselves; unmounting is the shell's Eject verb.

use super::Drive;
use std::path::PathBuf;
use std::process::Command;

const PROBE: &str = "Get-Volume | Where-Object { $_.DriveType -eq 'Removable' -and $_.DriveLetter } | ForEach-Object { '{0}|{1}|{2}' -f $_.DriveLetter, $_.FileSystemLabel, $_.UniqueId }";

pub(super) fn probe() -> Result<Vec<Drive>, String> {
    let output = powershell(PROBE).map_err(|err| format!("Could not list drives: {err}"))?;
    Ok(parse(&output))
}

pub(super) fn mount(drive: &Drive) -> Result<Option<PathBuf>, String> {
    Err(format!(
        "“{}” is mounted by Windows when it is plugged in.",
        drive.name
    ))
}

pub(super) fn unmount(drive: &Drive) -> Result<(), String> {
    let Some(path) = drive.path() else {
        return Err(format!("“{}” is not mounted.", drive.name));
    };
    let root = path.display().to_string().replace('\'', "''");
    let script = format!(
        "$shell = New-Object -ComObject Shell.Application; \
         $item = $shell.Namespace(17).ParseName('{root}'); \
         if ($item) {{ $item.InvokeVerb('Eject') }} else {{ exit 1 }}"
    );
    powershell(&script)
        .map(|_| ())
        .map_err(|err| format!("Could not unmount “{}”: {err}", drive.name))
}

fn powershell(script: &str) -> Result<String, String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let text = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(if text.is_empty() {
            "the command reported an error".to_owned()
        } else {
            text.to_owned()
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse(text: &str) -> Vec<Drive> {
    let mut drives = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '|');
        let Some(letter) = parts.next().filter(|letter| !letter.is_empty()) else {
            continue;
        };
        let label = parts.next().unwrap_or("");
        let unique = parts.next().unwrap_or("");
        let id = if unique.is_empty() {
            format!("volume:{letter}")
        } else {
            format!("volume:{unique}")
        };
        if drives.iter().any(|drive: &Drive| drive.id == id) {
            continue;
        }
        drives.push(Drive {
            id,
            name: if label.is_empty() {
                format!("{letter}: Drive")
            } else {
                label.to_owned()
            },
            device: format!("{letter}:"),
            mountpoint: Some(PathBuf::from(format!("{letter}:\\"))),
        });
    }
    drives
}
