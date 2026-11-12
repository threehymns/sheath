//! Windows removable drives through PowerShell.
//!
//! Windows gives a removable volume its drive letter when it is plugged in, so the list is the
//! mounted volumes themselves; unmounting is the shell's Eject verb. The verb returns before the
//! device is gone, so the worker waits for `Get-Volume` to stop reporting the letter.

use super::Drive;
use std::path::PathBuf;
use std::process::Command;

const PROBE: &str = "Get-Volume | Where-Object { $_.DriveType -eq 'Removable' -and $_.DriveLetter } | ForEach-Object { '{0}|{1}|{2}' -f $_.DriveLetter, $_.FileSystemLabel, $_.UniqueId }";

/// How long an Eject may take to remove the volume before the worker reports a failure.
const EJECT_TIMEOUT_SECONDS: u64 = 30;

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
    let letter = drive.device.trim_end_matches(':');
    if letter.is_empty() {
        return Err(format!("“{}” is not mounted.", drive.name));
    }
    // Eject is asynchronous: the verb starts the removal and returns. Keep the worker (and
    // therefore the row's spinner) alive until Windows stops listing the drive letter. The
    // timeout turns a stuck eject into an error instead of a spinner that never ends.
    let script = format!(
        "$shell = New-Object -ComObject Shell.Application; \
         $item = $shell.Namespace(17).ParseName('{root}'); \
         if (-not $item) {{ Write-Error 'The drive is no longer available.'; exit 1 }}; \
         $item.InvokeVerb('Eject'); \
         $deadline = (Get-Date).AddSeconds({EJECT_TIMEOUT_SECONDS}); \
         while ((Get-Date) -lt $deadline) {{ \
             Start-Sleep -Milliseconds 200; \
             if (-not (Get-Volume -DriveLetter '{letter}' -ErrorAction SilentlyContinue)) {{ exit 0 }} \
         }}; \
         Write-Error 'The drive did not disappear after ejecting.'; \
         exit 1"
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
