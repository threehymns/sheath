//! The resource table (`resources.arsc`). The manifest names the launcher icon by resource id,
//! and this maps that id to the file path that carries it, whatever the file is called.

use super::axml::{Pool, u16_at, u32_at};
use std::collections::HashMap;

const TABLE: u16 = 0x0002;
const PACKAGE: u16 = 0x0200;
const TYPE: u16 = 0x0201;

/// What an id resolves to: a file in the package, or a color the drawing paints with.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Path(String),
    Color(u32),
}

enum Raw {
    Str(String),
    Ref(u32),
    Color(u32),
    Other,
}

#[derive(Default)]
pub(super) struct Table {
    entries: HashMap<(u8, u8, u16), Vec<(u16, Raw)>>,
}

impl Table {
    #[cfg(test)]
    pub(super) fn empty() -> Table {
        Table::default()
    }

    /// The value an id names, preferring the densest configuration that resolves.
    pub(super) fn resolve(&self, id: u32) -> Option<Value> {
        self.lookup(id, 0)
    }

    fn lookup(&self, id: u32, depth: u8) -> Option<Value> {
        if depth > 8 {
            return None;
        }
        let key = ((id >> 24) as u8, (id >> 16) as u8, id as u16);
        let mut candidates: Vec<&(u16, Raw)> = self.entries.get(&key)?.iter().collect();
        candidates.sort_by_key(|(density, _)| std::cmp::Reverse(*density));
        for (_, raw) in candidates {
            match raw {
                Raw::Str(path) => return Some(Value::Path(path.clone())),
                Raw::Color(color) => return Some(Value::Color(*color)),
                Raw::Ref(next) if *next != id => {
                    if let Some(value) = self.lookup(*next, depth + 1) {
                        return Some(value);
                    }
                }
                Raw::Ref(_) | Raw::Other => {}
            }
        }
        None
    }
}

pub(super) fn parse(bytes: &[u8]) -> Option<Table> {
    if u16_at(bytes, 0)? != TABLE {
        return None;
    }
    let packages = u32_at(bytes, 8)?;
    let global = Pool::parse(bytes, 12)?;
    let mut table = Table::default();
    let mut at = 12 + global.size();
    let mut seen = 0;
    while seen < packages && at + 8 <= bytes.len() {
        let kind = u16_at(bytes, at)?;
        let size = u32_at(bytes, at + 4)? as usize;
        if size < 8 || at + size > bytes.len() {
            break;
        }
        if kind == PACKAGE {
            package(bytes, at, size, &global, &mut table);
            seen += 1;
        }
        at += size;
    }
    Some(table)
}

fn package(bytes: &[u8], at: usize, size: usize, global: &Pool, table: &mut Table) {
    let Some(header) = u16_at(bytes, at + 2).map(usize::from) else {
        return;
    };
    let Some(id) = bytes.get(at + 8).copied() else {
        return;
    };
    let end = at + size;
    let mut child = at + header;
    while child + 8 <= end {
        let Some(kind) = u16_at(bytes, child) else {
            return;
        };
        let Some(chunk_size) = u32_at(bytes, child + 4).map(|size| size as usize) else {
            return;
        };
        if chunk_size < 8 || child + chunk_size > end {
            return;
        }
        if kind == TYPE {
            type_chunk(bytes, child, id, global, table);
        }
        child += chunk_size;
    }
}

fn type_chunk(bytes: &[u8], at: usize, package: u8, global: &Pool, table: &mut Table) {
    let Some(header) = u16_at(bytes, at + 2).map(usize::from) else {
        return;
    };
    let Some(id) = bytes.get(at + 8).copied() else {
        return;
    };
    let Some(count) = u32_at(bytes, at + 12) else {
        return;
    };
    let Some(start) = u32_at(bytes, at + 16).map(|start| start as usize) else {
        return;
    };
    // The configuration starts after the chunk's fixed fields; density is a `u16` 14 bytes in.
    let (Some(config_size), Some(density)) = (u32_at(bytes, at + 20), u16_at(bytes, at + 34))
    else {
        return;
    };
    let density = if config_size >= 16 { density } else { 0 };
    for entry in 0..count as usize {
        let Some(offset) = u32_at(bytes, at + header + entry * 4) else {
            return;
        };
        if offset == u32::MAX {
            continue;
        }
        let Some((value, flags)) = value_at(bytes, at + start + offset as usize) else {
            continue;
        };
        // A bag is a style or a widget; a compact entry packs its value into its key field.
        if flags & 0x0001 != 0 || flags & 0x0008 != 0 {
            continue;
        }
        let raw = match *bytes.get(value + 3).unwrap_or(&0) {
            0x01 => match u32_at(bytes, value + 4) {
                Some(next) => Raw::Ref(next),
                None => Raw::Other,
            },
            0x03 => match u32_at(bytes, value + 4).and_then(|index| global.get(index as usize)) {
                Some(path) => Raw::Str(path.to_owned()),
                None => Raw::Other,
            },
            0x1c => u32_at(bytes, value + 4).map_or(Raw::Other, Raw::Color),
            0x1d => u32_at(bytes, value + 4).map_or(Raw::Other, |color| {
                Raw::Color(0xff00_0000 | (color & 0x00ff_ffff))
            }),
            _ => Raw::Other,
        };
        table
            .entries
            .entry((package, id, entry as u16))
            .or_default()
            .push((density, raw));
    }
}

/// The offset of a type's entry value: the entry header carries its own size.
fn value_at(bytes: &[u8], entry: usize) -> Option<(usize, u16)> {
    let size = u16_at(bytes, entry)? as usize;
    let flags = u16_at(bytes, entry + 2)?;
    Some((entry + size, flags))
}
