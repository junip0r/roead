use core::{mem::size_of, slice::SlicePattern};

use byte::{check_len, ctx, BytesExt, TryRead, BE, LE};

use super::NodeType;
use crate::{util::u24, Result};

impl TryRead<'_, ctx::Endian> for super::NodeType {
    fn try_read(bytes: &'_ [u8], ctx: ctx::Endian) -> byte::Result<(Self, usize)> {
        if bytes.is_empty() {
            Err(byte::Error::Incomplete)
        } else {
            match u8::try_read(bytes, ctx)?.0 {
                0x20 => Ok((Self::HashMap, 1)),
                0x21 => Ok((Self::ValueHashMap, 1)),
                0xa0 => Ok((Self::String, 1)),
                0xa1 => Ok((Self::Binary, 1)),
                0xa2 => Ok((Self::File, 1)),
                0xc0 => Ok((Self::Array, 1)),
                0xc1 => Ok((Self::Map, 1)),
                0xc2 => Ok((Self::StringTable, 1)),
                0xd0 => Ok((Self::Bool, 1)),
                0xd1 => Ok((Self::I32, 1)),
                0xd2 => Ok((Self::Float, 1)),
                0xd3 => Ok((Self::U32, 1)),
                0xd4 => Ok((Self::I64, 1)),
                0xd5 => Ok((Self::U64, 1)),
                0xd6 => Ok((Self::Double, 1)),
                0xff => Ok((Self::Null, 1)),
                _ => {
                    Err(byte::Error::BadInput {
                        err: "Invalid node type",
                    })
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Header {
    /// “BY” (big endian) or “YB” (little endian).
    magic: [u8; 2],
    /// Format version (1-7).
    version: u16,
    /// Offset to the hash key table, relative to start (usually 0x010)
    /// May be 0 if no hash nodes are used. Must be a string table node (0xc2).
    hash_key_table_offset: u32,
    /// Offset to the string table, relative to start. May be 0 if no strings
    /// are used. Must be a string table node (0xc2).
    string_table_offset: u32,
    /// Offset to the root node, relative to start. May be 0 if the document is
    /// totally empty. Must be either an array node (0xc0) or a hash node
    /// (0xc1).
    root_node_offset: u32,
}

impl TryRead<'_, ()> for Header {
    fn try_read(bytes: &'_ [u8], _ctx: ()) -> byte::Result<(Self, usize)> {
        check_len(bytes, size_of::<Header>())?;
        let endian = match &bytes[..2] {
            b"BY" => ctx::Endian::Big,
            b"YB" => ctx::Endian::Little,
            _ => {
                return Err(byte::Error::BadInput {
                    err: "Missing or invalid BYML magic",
                });
            }
        };
        let offset = &mut 2;
        Ok((
            Self {
                magic: [bytes[0], bytes[1]],
                version: bytes.read_with(offset, endian)?,
                hash_key_table_offset: bytes.read_with(offset, endian)?,
                string_table_offset: bytes.read_with(offset, endian)?,
                root_node_offset: bytes.read_with(offset, endian)?,
            },
            *offset,
        ))
    }
}

#[cfg(feature = "alloc")]
type Buffer<'a> = alloc::borrow::Cow<'a, [u8]>;
#[cfg(not(feature = "alloc"))]
type Buffer<'a> = &'a [u8];

#[derive(Debug, PartialEq)]
pub struct BymlIter<'a> {
    data: Buffer<'a>,
    endian: ctx::Endian,
    root_node_idx: Option<usize>,
}

#[derive(Debug, PartialEq)]
struct BymlStringTableReader<'a> {
    data: &'a [u8],
    len:  usize,
    ctx:  ctx::Endian,
}

struct BymlStringOffsetIterator<'a> {
    data:  &'a [u8],
    ctx:   ctx::Endian,
    len:   usize,
    index: usize,
}

impl Iterator for BymlStringOffsetIterator<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index * 4 > self.data.len() || self.index >= self.len {
            None
        } else {
            let offset = u32::try_read(&self.data[self.index * 4..], self.ctx)
                .ok()?
                .0;
            self.index += 1;
            Some(offset)
        }
    }
}

impl BymlStringTableReader<'_> {
    const TABLE_OFFSET: usize = 4;

    fn offset_iter(&self) -> BymlStringOffsetIterator<'_> {
        BymlStringOffsetIterator {
            data:  &self.data[Self::TABLE_OFFSET..],
            ctx:   self.ctx,
            len:   self.len,
            index: 0,
        }
    }

    #[inline]
    fn get(&self, index: u24) -> Option<&str> {
        let offset = self.offset_iter().nth(index.0 as usize)? as usize;
        self.data[offset..]
            .read_with(&mut 0, byte::ctx::Str::Delimiter(0))
            .ok()
    }

    #[inline]
    fn pos(&self, key: &str) -> Option<u32> {
        if self.len == 0 {
            None
        } else {
            let mut start = 0;
            let mut end = self.len;
            let mut index = start;
            while start < end {
                index = (start + end) / 2;
                let offset = u32::try_read(&self.data[Self::TABLE_OFFSET..], self.ctx)
                    .ok()?
                    .0 as usize;
                if offset > self.data.len() {
                    return None;
                }
                let string: &str = self.data[offset..]
                    .read_with(&mut 0, byte::ctx::Str::Delimiter(0))
                    .ok()?;
                match string.cmp(key) {
                    core::cmp::Ordering::Less => start = index + 1,
                    core::cmp::Ordering::Equal => return Some(offset as u32),
                    core::cmp::Ordering::Greater => end = index,
                }
            }
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct BymlContainerHeader {
    node_type: NodeType,
    len: usize,
}

impl TryRead<'_, ctx::Endian> for BymlContainerHeader {
    fn try_read(bytes: &'_ [u8], ctx: ctx::Endian) -> byte::Result<(Self, usize)> {
        check_len(bytes, 4)?;
        let offset = &mut 0;
        Ok((
            Self {
                node_type: bytes.read_with(offset, ctx)?,
                len: bytes.read_with::<u24>(offset, ctx)?.0 as usize,
            },
            4,
        ))
    }
}

#[derive(Debug)]
pub struct BymlMapIterator<'a> {
    data: &'a [u8],
    node: BymlContainerHeader,
    index: usize,
    invalid: bool,
    ctx: ctx::Endian,
}

impl BymlMapIterator<'_> {
    const TABLE_OFFSET: usize = 4;

    #[inline]
    fn len(&self) -> usize {
        self.node.len
    }

    fn new(node: BymlContainerHeader, data: &[u8], ctx: ctx::Endian) -> Self {
        Self {
            data,
            node,
            index: 0,
            invalid: data.len() < node.len * 8,
            ctx,
        }
    }
}

impl<'a> Iterator for BymlMapIterator<'a> {
    type Item = (&str, BymlData);

    fn next(&mut self) -> Option<Self::Item> {
        if self.invalid {
            None
        } else {
            let pair =
                BymlMapPair::try_read(&self.data[Self::TABLE_OFFSET + self.index * 8..], self.ctx)
                    .ok()?
                    .0;

            self.index += 1;
            Some(pair)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BymlData {
    value: u32,
    node_type: NodeType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BymlMapPair {
    key: u24,
    node_type: NodeType,
    value: u32,
}

impl TryRead<'_, ctx::Endian> for BymlMapPair {
    fn try_read(bytes: &'_ [u8], ctx: ctx::Endian) -> byte::Result<(Self, usize)> {
        check_len(bytes, 8)?;
        let offset = &mut 0;
        Ok((
            Self {
                key: bytes.read_with(offset, ctx)?,
                node_type: bytes.read_with(offset, ctx)?,
                value: bytes.read_with(offset, ctx)?,
            },
            8,
        ))
    }
}

impl<'a> BymlIter<'a> {
    pub fn new<I: Into<Buffer<'a>>>(data: I) -> Result<Self> {
        let data = data.into();
        let header = Header::try_read(&data, ())?.0;
        Ok(Self {
            data,
            endian: match &header.magic {
                b"BY" => BE,
                b"YB" => LE,
                _ => unreachable!(),
            },
            root_node_idx: (header.root_node_offset != 0)
                .then_some(header.root_node_offset as usize),
        })
    }

    #[inline]
    pub fn is_map(&self) -> bool {
        self.root_node_idx
            .and_then(|idx| {
                Some(
                    BymlContainerHeader::try_read(&self.data[idx..idx + 4], self.endian)
                        .ok()?
                        .0
                        .node_type
                        == NodeType::Map,
                )
            })
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_array(&self) -> bool {
        self.root_node_idx
            .and_then(|idx| {
                Some(
                    BymlContainerHeader::try_read(&self.data[idx..idx + 4], self.endian)
                        .ok()?
                        .0
                        .node_type
                        == NodeType::Array,
                )
            })
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_hash_map(&self) -> bool {
        self.root_node_idx
            .and_then(|idx| {
                Some(
                    BymlContainerHeader::try_read(&self.data[idx..idx + 4], self.endian)
                        .ok()?
                        .0
                        .node_type
                        == NodeType::HashMap,
                )
            })
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_value_hash_map(&self) -> bool {
        self.root_node_idx
            .and_then(|idx| {
                Some(
                    BymlContainerHeader::try_read(&self.data[idx..idx + 4], self.endian)
                        .ok()?
                        .0
                        .node_type
                        == NodeType::ValueHashMap,
                )
            })
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_container(&self) -> bool {
        self.root_node_idx
            .and_then(|idx| {
                Some(super::is_container_type(
                    BymlContainerHeader::try_read(&self.data[idx..idx + 4], self.endian)
                        .ok()?
                        .0
                        .node_type,
                ))
            })
            .unwrap_or(false)
    }
}
