//! A browser tab has no drives to mount.

use super::Drive;
use std::path::PathBuf;

pub(super) fn probe() -> Result<Vec<Drive>, String> {
    Ok(Vec::new())
}

pub(super) fn mount(_drive: &Drive) -> Result<Option<PathBuf>, String> {
    Err("A browser tab cannot mount drives.".to_owned())
}

pub(super) fn unmount(_drive: &Drive) -> Result<(), String> {
    Err("A browser tab cannot unmount drives.".to_owned())
}
