//! Android's binary XML: the manifest, an adaptive icon definition and a vector drawable all
//! arrive as a string pool followed by start and end element chunks. Only the tree and its typed
//! attributes are read here.

/// A parsed document. The root is the one top-level element.
pub(super) struct Document {
    pub(super) root: Node,
}

pub(super) struct Node {
    pub(super) name: String,
    pub(super) attrs: Vec<Attr>,
    pub(super) children: Vec<Node>,
}

pub(super) struct Attr {
    pub(super) name: String,
    pub(super) value: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Value {
    Str(String),
    Ref(u32),
    Int(i32),
    Float(f32),
    Other,
}

const XML: u16 = 0x0003;
const STRING_POOL: u16 = 0x0001;
const START_ELEMENT: u16 = 0x0102;
const END_ELEMENT: u16 = 0x0103;
const UTF8_FLAG: u32 = 0x100;

impl Node {
    pub(super) fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|child| child.name == name)
    }

    pub(super) fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs
            .iter()
            .find(|attr| attr.name == name)
            .map(|attr| &attr.value)
    }
}

impl Value {
    pub(super) fn reference(&self) -> Option<u32> {
        match self {
            Value::Ref(id) => Some(*id),
            _ => None,
        }
    }
}

pub(super) fn parse(bytes: &[u8]) -> Option<Document> {
    if u16_at(bytes, 0)? != XML {
        return None;
    }
    let end = (u32_at(bytes, 4)? as usize).min(bytes.len());
    let mut at = u16_at(bytes, 2)? as usize;
    let mut pool: Option<Pool> = None;
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    while at.checked_add(8)? <= end {
        let kind = u16_at(bytes, at)?;
        let header = u16_at(bytes, at + 2)? as usize;
        let size = u32_at(bytes, at + 4)? as usize;
        if size < 8 || at + size > end {
            break;
        }
        match kind {
            STRING_POOL if pool.is_none() => pool = Some(Pool::parse(bytes, at)?),
            START_ELEMENT => {
                let pool = pool.as_ref()?;
                stack.push(element(bytes, at, header, pool)?);
            }
            END_ELEMENT => {
                let node = stack.pop()?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            _ => {}
        }
        at += size;
    }
    // A truncated document still has its elements; keep what was read.
    if root.is_none() && !stack.is_empty() {
        root = Some(stack.remove(0));
    }
    Some(Document { root: root? })
}

fn element(bytes: &[u8], at: usize, header: usize, pool: &Pool) -> Option<Node> {
    let name = pool.get(u32_at(bytes, at + 20)? as usize)?.to_owned();
    let start = u16_at(bytes, at + 24)? as usize;
    let step = u16_at(bytes, at + 26)? as usize;
    let count = u16_at(bytes, at + 28)? as usize;
    let mut attrs = Vec::with_capacity(count);
    for index in 0..count {
        let attr = at + header + start + index * step;
        let name = pool.get(u32_at(bytes, attr + 4)? as usize)?.to_owned();
        let data = u32_at(bytes, attr + 16)?;
        let value = match *bytes.get(attr + 15)? {
            0x01 | 0x02 => Value::Ref(data),
            0x03 => Value::Str(pool.get(data as usize)?.to_owned()),
            0x04 => Value::Float(f32::from_bits(data)),
            0x10..=0x12 | 0x1c..=0x1f => Value::Int(data as i32),
            _ => Value::Other,
        };
        attrs.push(Attr { name, value });
    }
    Some(Node {
        name,
        attrs,
        children: Vec::new(),
    })
}

pub(super) fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

pub(super) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The string pool shared by AXML chunks and the resource table's global names.
pub(super) struct Pool {
    strings: Vec<String>,
    size: usize,
}

impl Pool {
    pub(super) fn parse(bytes: &[u8], at: usize) -> Option<Pool> {
        if u16_at(bytes, at)? != STRING_POOL {
            return None;
        }
        let header = u16_at(bytes, at + 2)? as usize;
        let size = u32_at(bytes, at + 4)? as usize;
        let count = u32_at(bytes, at + 8)? as usize;
        let flags = u32_at(bytes, at + 16)?;
        let start = u32_at(bytes, at + 20)? as usize;
        let mut strings = Vec::with_capacity(count);
        for index in 0..count {
            let offset = u32_at(bytes, at + header + index * 4)? as usize;
            strings.push(text(bytes, at + start + offset, flags & UTF8_FLAG != 0)?);
        }
        Some(Pool { strings, size })
    }

    pub(super) fn get(&self, index: usize) -> Option<&str> {
        self.strings.get(index).map(String::as_str)
    }

    pub(super) fn size(&self) -> usize {
        self.size
    }
}

fn text(bytes: &[u8], start: usize, utf8: bool) -> Option<String> {
    if utf8 {
        let (_, at) = length8(bytes, start)?;
        let (length, at) = length8(bytes, at)?;
        let slice = bytes.get(at..at + length)?;
        Some(std::str::from_utf8(slice).ok()?.to_owned())
    } else {
        let (length, at) = length16(bytes, start)?;
        let slice = bytes.get(at..at.checked_add(length.checked_mul(2)?)?)?;
        let units: Vec<u16> = slice
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .collect();
        String::from_utf16(&units).ok()
    }
}

/// A UTF-8 string's two lengths. Both use the high bit as a second length byte.
fn length8(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let first = *bytes.get(at)?;
    if first & 0x80 == 0 {
        Some((first as usize, at + 1))
    } else {
        Some((
            ((first & 0x7f) as usize) << 8 | *bytes.get(at + 1)? as usize,
            at + 2,
        ))
    }
}

/// A UTF-16 string's length, with the high bit as a second length word.
fn length16(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let first = u16_at(bytes, at)?;
    if first & 0x8000 == 0 {
        Some((first as usize, at + 2))
    } else {
        Some((
            ((first & 0x7fff) as usize) << 16 | u16_at(bytes, at + 2)? as usize,
            at + 4,
        ))
    }
}
