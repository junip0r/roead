use core::mem::size_of;

use byte::{check_len, ctx, BytesExt, TryRead, BE, LE};

use super::NodeType;
use crate::{util::u24, Result};

#[cfg(feature = "alloc")]
impl super::Byml {
    pub fn from_binary(data: impl AsRef<[u8]>) -> Result<super::Byml> {
        #[cfg(feature = "yaz0")]
        {
            if data.as_ref().starts_with(b"Yaz0") {
                return BymlIter::new(crate::yaz0::decompress(data)?)?.try_into();
            }
        }
        BymlIter::new(data.as_ref())?.try_into()
    }
}

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

impl<'a> BymlStringTableReader<'a> {
    const TABLE_OFFSET: usize = 4;

    #[inline]
    fn new(data: &'a [u8], ctx: ctx::Endian) -> Result<Self> {
        let node_type = NodeType::try_read(data, ctx)?.0;
        if node_type != NodeType::StringTable {
            Err(crate::Error::InvalidData("Invalid string table"))
        } else {
            let len = u24::try_read(&data[1..], ctx)?.0;
            Ok(Self {
                data,
                len: len.0 as usize,
                ctx,
            })
        }
    }

    #[inline]
    fn offset_iter(&self) -> BymlStringOffsetIterator<'_> {
        BymlStringOffsetIterator {
            data:  &self.data[Self::TABLE_OFFSET..],
            ctx:   self.ctx,
            len:   self.len,
            index: 0,
        }
    }

    #[inline]
    fn get<'s>(&'s self, index: u24) -> Option<&'a str> {
        let offset = self.offset_iter().nth(index.0 as usize)? as usize;
        self.data[offset..]
            .read_with(&mut 0, byte::ctx::Str::Delimiter(0))
            .ok()
    }

    fn pos(&self, key: &str) -> Option<u32> {
        if self.len == 0 {
            None
        } else {
            let mut start = 0;
            let mut end = self.len;
            let mut index;
            while start < end {
                index = (start + end) / 2;
                let offset = u32::try_read(&self.data[Self::TABLE_OFFSET + index * 4..], self.ctx)
                    .ok()?
                    .0 as usize;
                if offset > self.data.len() {
                    return None;
                }
                let string: &str = self.data[offset..]
                    .read_with(&mut 0, byte::ctx::Str::Delimiter(0))
                    .ok()?;
                match string.cmp(key) {
                    core::cmp::Ordering::Equal => return Some(index as u32),
                    core::cmp::Ordering::Less => start = index + 1,
                    core::cmp::Ordering::Greater => end = index,
                }
            }
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    len: usize,
    strings: BymlStringTableReader<'a>,
    index: usize,
    invalid: bool,
    ctx: ctx::Endian,
}

impl<'a> BymlMapIterator<'a> {
    const TABLE_OFFSET: usize = 4;

    #[inline]
    fn new(
        node: BymlContainerHeader,
        data: &'a [u8],
        strings: BymlStringTableReader<'a>,
        ctx: ctx::Endian,
    ) -> Self {
        Self {
            data,
            len: node.len,
            strings,
            index: 0,
            invalid: data.len() < node.len * 8,
            ctx,
        }
    }

    fn find_by_key(&self, key_index: u24) -> Option<BymlNode> {
        if self.invalid {
            None
        } else {
            let mut start = 0;
            let mut end = self.len;
            let mut index;
            while start < end {
                index = (start + end) / 2;
                let pair =
                    BymlMapPair::try_read(&self.data[Self::TABLE_OFFSET + index * 8..], self.ctx)
                        .ok()?
                        .0;
                match pair.key.cmp(&key_index) {
                    core::cmp::Ordering::Equal => {
                        return Some(BymlNode::new(pair.value, pair.node_type));
                    }
                    core::cmp::Ordering::Less => {
                        start = index + 1;
                    }
                    core::cmp::Ordering::Greater => {
                        end = index;
                    }
                }
            }
            None
        }
    }
}

impl<'a> Iterator for BymlMapIterator<'a> {
    type Item = (&'a str, BymlNode);

    fn next(&mut self) -> Option<Self::Item> {
        if self.invalid {
            None
        } else {
            let pair =
                BymlMapPair::try_read(&self.data[Self::TABLE_OFFSET + self.index * 8..], self.ctx)
                    .ok()?
                    .0;
            let key = self.strings.get(pair.key)?;
            self.index += 1;
            Some((
                unsafe { core::mem::transmute(key) },
                BymlNode::new(pair.value, pair.node_type),
            ))
        }
    }
}

#[derive(Debug)]
pub struct BymlArrayIterator<'a> {
    data: &'a [u8],
    node: BymlContainerHeader,
    index: usize,
    invalid: bool,
    ctx: ctx::Endian,
}

impl<'a> BymlArrayIterator<'a> {
    const TYPE_TABLE_OFFSET: usize = 4;

    #[inline]
    fn new(node: BymlContainerHeader, data: &'a [u8], ctx: ctx::Endian) -> Self {
        Self {
            data,
            node,
            index: 0,
            invalid: data.len() < node.len * 5 + 4,
            ctx,
        }
    }

    #[inline]
    fn value_start(&self) -> usize {
        crate::util::align((Self::TYPE_TABLE_OFFSET + self.node.len) as u32, 4) as usize
    }
}

impl<'a> Iterator for BymlArrayIterator<'a> {
    type Item = BymlNode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.invalid {
            None
        } else {
            let node_type =
                NodeType::try_read(&self.data[Self::TYPE_TABLE_OFFSET + self.index..], self.ctx)
                    .ok()?
                    .0;
            let value = u32::try_read(&self.data[self.value_start() + self.index * 4..], self.ctx)
                .ok()?
                .0;
            self.index += 1;
            Some(BymlNode::new(value, node_type))
        }
    }
}

#[derive(Debug)]
pub struct BymlHashMapIterator<'a> {
    data: &'a [u8],
    len: usize,
    index: usize,
    invalid: bool,
    value: bool,
    ctx: ctx::Endian,
}

impl<'a> BymlHashMapIterator<'a> {
    const TABLE_OFFSET: usize = 4;

    #[inline]
    fn new(node: BymlContainerHeader, data: &'a [u8], value: bool, ctx: ctx::Endian) -> Self {
        Self {
            data,
            index: 0,
            len: node.len,
            value,
            invalid: data.len() < Self::TABLE_OFFSET + node.len * 9,
            ctx,
        }
    }

    fn find_by_key(&self, key: u32) -> Option<BymlNode> {
        if self.invalid {
            None
        } else {
            let mut start = 0;
            let mut end = self.len;
            let mut index;
            while start < end {
                index = (start + end) / 2;
                let size = if self.value { 12 } else { 8 };
                let hash = u32::try_read(&self.data[Self::TABLE_OFFSET + index * size..], self.ctx)
                    .ok()?
                    .0;
                match hash.cmp(&key) {
                    core::cmp::Ordering::Equal => {
                        let value = u32::try_read(
                            &self.data[Self::TABLE_OFFSET + index * size + 4..],
                            self.ctx,
                        )
                        .ok()?
                        .0;
                        let node_type = NodeType::try_read(
                            &self.data[Self::TABLE_OFFSET + self.len * size + index..],
                            self.ctx,
                        )
                        .ok()?
                        .0;
                        return Some(BymlNode::new(value, node_type));
                    }
                    core::cmp::Ordering::Less => {
                        start = index + 1;
                    }
                    core::cmp::Ordering::Greater => {
                        end = index;
                    }
                }
            }
            None
        }
    }
}

impl<'a> Iterator for BymlHashMapIterator<'a> {
    type Item = (u32, BymlNode);

    fn next(&mut self) -> Option<Self::Item> {
        if self.invalid {
            None
        } else {
            let size = if self.value { 12 } else { 8 };
            let hash = u32::try_read(
                &self.data[Self::TABLE_OFFSET + self.index * size..],
                self.ctx,
            )
            .ok()?
            .0;
            let value = u32::try_read(
                &self.data[Self::TABLE_OFFSET + self.index * size + 4..],
                self.ctx,
            )
            .ok()?
            .0;
            let node_type = NodeType::try_read(
                &self.data[Self::TABLE_OFFSET + self.len * size + self.index..],
                self.ctx,
            )
            .ok()?
            .0;
            self.index += 1;
            Some((hash, BymlNode::new(value, node_type)))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BymlNode {
    HashMap { offset: usize },
    ValueHashMap { offset: usize },
    String { index: u32 },
    Binary { offset: usize },
    File { offset: usize },
    Array { offset: usize },
    Map { offset: usize },
    StringTable { offset: usize },
    Bool(bool),
    I32(i32),
    Float(f32),
    U32(u32),
    I64 { offset: usize },
    U64 { offset: usize },
    Double { offset: usize },
    Null,
}

impl BymlNode {
    pub fn new(value: u32, node_type: NodeType) -> Self {
        match node_type {
            NodeType::String => Self::String { index: value },
            NodeType::HashMap => {
                Self::HashMap {
                    offset: value as usize,
                }
            }
            NodeType::ValueHashMap => {
                Self::ValueHashMap {
                    offset: value as usize,
                }
            }
            NodeType::Binary => {
                Self::Binary {
                    offset: value as usize,
                }
            }
            NodeType::File => {
                Self::File {
                    offset: value as usize,
                }
            }
            NodeType::Array => {
                Self::Array {
                    offset: value as usize,
                }
            }
            NodeType::Map => {
                Self::Map {
                    offset: value as usize,
                }
            }
            NodeType::StringTable => {
                Self::StringTable {
                    offset: value as usize,
                }
            }
            NodeType::I64 => {
                Self::I64 {
                    offset: value as usize,
                }
            }
            NodeType::U64 => {
                Self::U64 {
                    offset: value as usize,
                }
            }
            NodeType::Double => {
                Self::Double {
                    offset: value as usize,
                }
            }
            NodeType::Bool => Self::Bool(value == 1),
            NodeType::I32 => Self::I32(value as i32),
            NodeType::Float => Self::Float(value as f32),
            NodeType::U32 => Self::U32(value),
            NodeType::Null => Self::Null,
        }
    }
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
    fn header(&self) -> Result<Header> {
        Ok(Header::try_read(&self.data, ())?.0)
    }

    #[inline]
    fn key_table(&self) -> Result<BymlStringTableReader> {
        let keys_offset = self.header()?.hash_key_table_offset as usize;
        BymlStringTableReader::new(&self.data[keys_offset..], self.endian)
    }

    #[inline]
    fn string_table(&self) -> Result<BymlStringTableReader> {
        let string_offset = self.header()?.string_table_offset as usize;
        BymlStringTableReader::new(&self.data[string_offset..], self.endian)
    }

    #[inline]
    fn parse_container(&self, offset: usize) -> Result<BymlContainerHeader> {
        Ok(BymlContainerHeader::try_read(&self.data[offset..], self.endian)?.0)
    }

    #[inline]
    fn root_node(&self) -> Option<BymlContainerHeader> {
        self.root_node_idx
            .and_then(|idx| self.parse_container(idx).ok())
    }

    #[inline]
    pub fn is_map(&self) -> bool {
        self.root_node()
            .map(|n| n.node_type == NodeType::Map)
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_array(&self) -> bool {
        self.root_node()
            .map(|n| n.node_type == NodeType::Array)
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_hash_map(&self) -> bool {
        self.root_node()
            .map(|n| n.node_type == NodeType::HashMap)
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_value_hash_map(&self) -> bool {
        self.root_node()
            .map(|n| n.node_type == NodeType::ValueHashMap)
            .unwrap_or(false)
    }

    #[inline]
    pub fn is_container(&self) -> bool {
        self.root_node()
            .map(|n| super::is_container_type(n.node_type))
            .unwrap_or(false)
    }

    #[inline]
    fn get_key_index(&self, key: &str) -> Option<u32> {
        let keys = self.key_table().ok()?;
        keys.pos(key)
    }

    #[inline]
    pub fn get<'i, I: Into<super::BymlIndex<'i>>>(&self, key: I) -> Option<BymlNode> {
        match key.into() {
            super::BymlIndex::ArrayIdx(i) => self.iter_as_array().and_then(|mut a| a.nth(i)),
            super::BymlIndex::StringIdx(s) => {
                let index = self.get_key_index(s)?;
                self.iter_as_map()
                    .and_then(|map| map.find_by_key(u24(index)))
            }
            super::BymlIndex::HashIdx(h) => {
                self.iter_as_hash_map()
                    .and_then(|m| m.find_by_key(h))
                    .or_else(|| self.iter_as_value_hash_map().and_then(|m| m.find_by_key(h)))
            }
        }
    }

    #[inline]
    pub fn get_from<'i, I: Into<super::BymlIndex<'i>>>(
        &self,
        node: BymlNode,
        key: I,
    ) -> Option<BymlNode> {
        match key.into() {
            super::BymlIndex::ArrayIdx(i) => self.iter_array_data(node).and_then(|mut a| a.nth(i)),
            super::BymlIndex::StringIdx(s) => {
                let index = self.get_key_index(s)?;
                self.iter_map_data(node)
                    .and_then(|map| map.find_by_key(u24(index)))
            }
            super::BymlIndex::HashIdx(h) => {
                self.iter_hash_map_data(node)
                    .and_then(|m| m.find_by_key(h))
                    .or_else(|| {
                        self.iter_value_hash_map_data(node)
                            .and_then(|m| m.find_by_key(h))
                    })
            }
        }
    }

    #[inline]
    pub fn iter_as_array(&self) -> Option<BymlArrayIterator<'_>> {
        if self.is_array() {
            let node = unsafe { self.root_node().unwrap_unchecked() };
            Some(BymlArrayIterator::new(
                node,
                &self.data[unsafe { self.root_node_idx.unwrap_unchecked() }..],
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_as_map(&self) -> Option<BymlMapIterator<'_>> {
        if self.is_map() {
            let node = unsafe { self.root_node().unwrap_unchecked() };
            Some(BymlMapIterator::new(
                node,
                &self.data[unsafe { self.root_node_idx.unwrap_unchecked() }..],
                self.key_table().ok()?,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_as_hash_map(&self) -> Option<BymlHashMapIterator<'_>> {
        if self.is_hash_map() {
            let node = unsafe { self.root_node().unwrap_unchecked() };
            Some(BymlHashMapIterator::new(
                node,
                &self.data[unsafe { self.root_node_idx.unwrap_unchecked() }..],
                false,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_as_value_hash_map(&self) -> Option<BymlHashMapIterator<'_>> {
        if self.is_value_hash_map() {
            let node = unsafe { self.root_node().unwrap_unchecked() };
            Some(BymlHashMapIterator::new(
                node,
                &self.data[unsafe { self.root_node_idx.unwrap_unchecked() }..],
                true,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn get_string_data(&self, data: BymlNode) -> Option<&str> {
        if let BymlNode::String { index } = data {
            let strings_offset = self.header().ok()?.string_table_offset as usize;
            let strings = self.string_table().ok()?;
            strings.get(u24(index))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_map_data(&self, data: BymlNode) -> Option<BymlMapIterator<'_>> {
        if let BymlNode::Map { offset } = data {
            let node = self.parse_container(offset).ok()?;
            Some(BymlMapIterator::new(
                node,
                &self.data[offset..],
                self.key_table().ok()?,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_array_data(&self, data: BymlNode) -> Option<BymlArrayIterator<'_>> {
        if let BymlNode::Array { offset } = data {
            let node = self.parse_container(offset).ok()?;
            Some(BymlArrayIterator::new(
                node,
                &self.data[offset..],
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_hash_map_data(&self, data: BymlNode) -> Option<BymlHashMapIterator<'_>> {
        if let BymlNode::HashMap { offset } = data {
            let node = self.parse_container(offset).ok()?;
            Some(BymlHashMapIterator::new(
                node,
                &self.data[offset..],
                false,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_value_hash_map_data(&self, data: BymlNode) -> Option<BymlHashMapIterator<'_>> {
        if let BymlNode::ValueHashMap { offset } = data {
            let node = self.parse_container(offset).ok()?;
            Some(BymlHashMapIterator::new(
                node,
                &self.data[offset..],
                true,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn get_i64_data(&self, data: BymlNode) -> Option<i64> {
        if let BymlNode::I64 { offset } = data {
            let value = i64::try_read(&self.data[offset..], self.endian).ok()?.0;
            Some(value)
        } else {
            None
        }
    }

    #[inline]
    pub fn get_u64_data(&self, data: BymlNode) -> Option<u64> {
        if let BymlNode::U64 { offset } = data {
            let value = u64::try_read(&self.data[offset..], self.endian).ok()?.0;
            Some(value)
        } else {
            None
        }
    }

    #[inline]
    pub fn get_double_data(&self, data: BymlNode) -> Option<f64> {
        if let BymlNode::Double { offset } = data {
            let value = f64::try_read(&self.data[offset..], self.endian).ok()?.0;
            Some(value)
        } else {
            None
        }
    }

    pub fn get_binary_data(&self, data: BymlNode) -> Option<&[u8]> {
        if let BymlNode::Binary { offset } = data {
            let data = &self.data[offset..];
            let size = u32::try_read(data, self.endian).ok()?.0 as usize;
            if data.len() >= size + 4 {
                Some(unsafe { core::slice::from_raw_parts(data[4..].as_ptr(), size) })
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_file_data(&self, data: BymlNode) -> Option<&[u8]> {
        if let BymlNode::Binary { offset } = data {
            let data = &self.data[offset..];
            let size = u32::try_read(data, self.endian).ok()?.0 as usize;
            if data.len() >= size + 8 {
                Some(unsafe { core::slice::from_raw_parts(data[8..].as_ptr(), size) })
            } else {
                None
            }
        } else {
            None
        }
    }

    #[cfg(feature = "alloc")]
    fn node_to_byml(&self, node: BymlNode) -> Result<super::Byml> {
        match node {
            BymlNode::HashMap { .. } => {
                self.iter_hash_map_data(node)
                    .ok_or(byte::Error::BadInput {
                        err: "Invalid hash map node",
                    })?
                    .map(|(k, v)| self.node_to_byml(v).map(|v| (k, v)))
                    .collect::<Result<_>>()
                    .map(super::Byml::HashMap)
            }
            BymlNode::ValueHashMap { .. } => {
                self.iter_value_hash_map_data(node)
                    .ok_or(byte::Error::BadInput {
                        err: "Invalid value hash map node",
                    })?
                    .map(|(k, v)| self.node_to_byml(v).map(|v| (k, v)))
                    .collect::<Result<_>>()
                    .map(super::Byml::HashMap)
            }
            BymlNode::String { .. } => {
                Ok(super::Byml::String(
                    self.get_string_data(node)
                        .ok_or(byte::Error::BadInput {
                            err: "Invalid string node",
                        })?
                        .into(),
                ))
            }
            BymlNode::Binary { .. } => {
                Ok(super::Byml::BinaryData(
                    self.get_binary_data(node)
                        .ok_or(byte::Error::BadInput {
                            err: "Invalid binary node",
                        })?
                        .into(),
                ))
            }
            BymlNode::File { .. } => {
                Ok(super::Byml::FileData(
                    self.get_file_data(node)
                        .ok_or(byte::Error::BadInput {
                            err: "Invalid file node",
                        })?
                        .into(),
                ))
            }
            BymlNode::Array { .. } => {
                self.iter_array_data(node)
                    .ok_or(byte::Error::BadInput {
                        err: "Invalid array node",
                    })?
                    .map(|node| self.node_to_byml(node))
                    .collect::<Result<_>>()
            }
            BymlNode::Map { .. } => {
                self.iter_map_data(node)
                    .ok_or(byte::Error::BadInput {
                        err: "Invalid map node",
                    })?
                    .map(|(k, v)| self.node_to_byml(v).map(|v| (k.into(), v)))
                    .collect::<Result<_>>()
                    .map(super::Byml::Map)
            }
            BymlNode::StringTable { .. } => unimplemented!(),
            BymlNode::I64 { .. } => {
                Ok(super::Byml::I64(
                    self.get_i64_data(node)
                        .ok_or(byte::Error::BadInput {
                            err: "Invalid i64 node",
                        })?
                        .into(),
                ))
            }
            BymlNode::U64 { .. } => {
                Ok(super::Byml::U64(
                    self.get_u64_data(node)
                        .ok_or(byte::Error::BadInput {
                            err: "Invalid u64 node",
                        })?
                        .into(),
                ))
            }
            BymlNode::Double { .. } => {
                Ok(super::Byml::Double(
                    self.get_double_data(node)
                        .ok_or(byte::Error::BadInput {
                            err: "Invalid double node",
                        })?
                        .into(),
                ))
            }
            BymlNode::Null => Ok(super::Byml::Null),
            BymlNode::Bool(v) => Ok(super::Byml::Bool(v)),
            BymlNode::I32(v) => Ok(super::Byml::I32(v)),
            BymlNode::Float(v) => Ok(super::Byml::Float(v)),
            BymlNode::U32(v) => Ok(super::Byml::U32(v)),
        }
    }
}

#[cfg(feature = "alloc")]
impl TryFrom<&BymlIter<'_>> for super::Byml {
    type Error = crate::Error;

    fn try_from(value: &BymlIter) -> core::result::Result<Self, Self::Error> {
        value
            .root_node()
            .map(|header| {
                let node = BymlNode::new(
                    unsafe { value.root_node_idx.unwrap_unchecked() } as u32,
                    header.node_type,
                );
                value.node_to_byml(node)
            })
            .transpose()
            .map(|by| by.unwrap_or(super::Byml::Null))
    }
}

#[cfg(feature = "alloc")]
impl TryFrom<BymlIter<'_>> for super::Byml {
    type Error = crate::Error;

    fn try_from(value: BymlIter<'_>) -> core::result::Result<Self, Self::Error> {
        (&value).try_into()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse() {
        let data =
            include_bytes!("../../test/byml/Mrg_01e57204_MrgD100_B4-B3-B2-1A90E17A.bcett.byml");
        let parser = super::BymlIter::new(data.as_slice()).unwrap();
        assert_eq!(parser.header().unwrap().root_node_offset, 264);
        assert_eq!(parser.root_node().unwrap(), super::BymlContainerHeader {
            len: 1,
            node_type: crate::byml::NodeType::Map,
        });
    }

    #[test]
    fn iter() {
        let data = include_bytes!("../../test/byml/USen.byml");
        let parser = super::BymlIter::new(data.as_slice()).unwrap();
        assert!(parser.is_hash_map());
        for (_, v) in parser.iter_as_hash_map().unwrap() {
            assert!(matches!(v, super::BymlNode::Map { .. }));
            let hash = parser.get_from(v, "Hash").unwrap();
            assert!(matches!(hash, super::BymlNode::U32(_)));
        }
        let second = parser.get(4253374u32).unwrap();
        assert_eq!(
            parser.get_from(second, "Hash").unwrap(),
            super::BymlNode::U32(0xD548098A)
        );
        let third = parser.get(7458797u32).unwrap();
        let channel_info = parser.get_from(third, "ChannelInfo").unwrap();
        let first = parser.get_from(channel_info, 0).unwrap();
        let adpcm_context = parser.get_from(first, "AdpcmContext").unwrap();
        let bin = parser.get_binary_data(adpcm_context).unwrap();
        assert_eq!(&bin, b"\0\0\0\0\0\0");
    }
}
