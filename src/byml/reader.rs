use core::mem::size_of;

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
    strings: BymlStringTableReader<'a>,
    node: BymlContainerHeader,
    index: usize,
    invalid: bool,
    ctx: ctx::Endian,
}

impl<'a> BymlMapIterator<'a> {
    const TABLE_OFFSET: usize = 4;

    #[inline]
    fn len(&self) -> usize {
        self.node.len
    }

    #[inline]
    fn new(
        node: BymlContainerHeader,
        data: &'a [u8],
        strings: BymlStringTableReader<'a>,
        ctx: ctx::Endian,
    ) -> Self {
        Self {
            data,
            strings,
            node,
            index: 0,
            invalid: data.len() < node.len * 8,
            ctx,
        }
    }
}

impl<'a> Iterator for BymlMapIterator<'a> {
    type Item = (&'a str, BymlData);

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
            Some((unsafe { core::mem::transmute(key) }, BymlData {
                node_type: pair.node_type,
                value: pair.value,
            }))
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
}

impl Iterator for BymlArrayIterator<'_> {
    type Item = BymlData;

    fn next(&mut self) -> Option<Self::Item> {
        if self.invalid {
            None
        } else {
            let node_type =
                NodeType::try_read(&self.data[Self::TYPE_TABLE_OFFSET + self.index..], self.ctx)
                    .ok()?
                    .0;
            let value = u32::try_read(
                &self.data[Self::TYPE_TABLE_OFFSET + self.node.len + self.index * 4..],
                self.ctx,
            )
            .ok()?
            .0;
            self.index += 1;
            Some(BymlData { value, node_type })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BymlData {
    value: u32,
    node_type: NodeType,
}

impl BymlData {
    #[inline]
    pub fn node_type(&self) -> NodeType {
        self.node_type
    }

    #[inline]
    pub fn get_bool(&self) -> Option<bool> {
        (self.node_type == NodeType::Bool).then_some(self.value != 0)
    }

    #[inline]
    pub fn as_bool(&self) -> bool {
        self.value != 0
    }

    #[inline]
    pub fn get_i32(&self) -> Option<i32> {
        (self.node_type == NodeType::I32).then_some(self.value as i32)
    }

    #[inline]
    pub fn as_i32(&self) -> i32 {
        self.value as i32
    }

    #[inline]
    pub fn get_float(&self) -> Option<f32> {
        (self.node_type == NodeType::Float).then_some(self.value as f32)
    }

    #[inline]
    pub fn as_float(&self) -> f32 {
        self.value as f32
    }

    #[inline]
    pub fn get_u32(&self) -> Option<u32> {
        (self.node_type == NodeType::U32).then_some(self.value)
    }

    #[inline]
    pub fn as_u32(&self) -> u32 {
        self.value
    }

    #[inline]
    pub fn is_map(&self) -> bool {
        self.node_type == NodeType::Map
    }

    #[inline]
    pub fn is_array(&self) -> bool {
        self.node_type == NodeType::Array
    }

    #[inline]
    pub fn is_hash_map(&self) -> bool {
        self.node_type == NodeType::HashMap
    }

    #[inline]
    pub fn is_value_hash_map(&self) -> bool {
        self.node_type == NodeType::ValueHashMap
    }

    #[inline]
    pub fn is_container(&self) -> bool {
        super::is_container_type(self.node_type)
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        self.node_type == NodeType::String
    }

    #[inline]
    pub fn is_binary(&self) -> bool {
        self.node_type == NodeType::Binary
    }

    #[inline]
    pub fn is_file(&self) -> bool {
        self.node_type == NodeType::File
    }

    #[inline]
    pub fn is_i64(&self) -> bool {
        self.node_type == NodeType::I64
    }

    #[inline]
    pub fn is_u64(&self) -> bool {
        self.node_type == NodeType::U64
    }

    #[inline]
    pub fn is_double(&self) -> bool {
        self.node_type == NodeType::Double
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.node_type == NodeType::Null
    }

    #[inline]
    pub fn as_offset(&self) -> usize {
        self.value as usize
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
            let keys_offset = self.header().ok()?.hash_key_table_offset as usize;
            let strings =
                BymlStringTableReader::new(&self.data[keys_offset..], self.endian).ok()?;
            Some(BymlMapIterator::new(
                node,
                &self.data[unsafe { self.root_node_idx.unwrap_unchecked() }..],
                strings,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn get_string_data(&self, data: BymlData) -> Option<&str> {
        data.is_string()
            .then(|| {
                let strings_offset = self.header().ok()?.string_table_offset as usize;
                let strings =
                    BymlStringTableReader::new(&self.data[strings_offset..], self.endian).ok()?;
                strings.get(u24(data.value))
            })
            .flatten()
    }

    #[inline]
    pub fn iter_map_data(&self, data: BymlData) -> Option<BymlMapIterator<'_>> {
        if data.node_type == NodeType::Map {
            let node = self.parse_container(data.as_offset()).ok()?;
            let keys_offset = self.header().ok()?.hash_key_table_offset as usize;
            let strings =
                BymlStringTableReader::new(&self.data[keys_offset..], self.endian).ok()?;
            Some(BymlMapIterator::new(
                node,
                &self.data[data.as_offset()..],
                strings,
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn iter_array_data(&self, data: BymlData) -> Option<BymlArrayIterator<'_>> {
        if data.node_type == NodeType::Array {
            let node = self.parse_container(data.as_offset()).ok()?;
            Some(BymlArrayIterator::new(
                node,
                &self.data[data.as_offset()..],
                self.endian,
            ))
        } else {
            None
        }
    }

    #[inline]
    pub fn get_i64_data(&self, data: BymlData) -> Option<i64> {
        if data.node_type == NodeType::I64 {
            let value = i64::try_read(&self.data[data.as_offset()..], self.endian)
                .ok()?
                .0;
            Some(value)
        } else {
            None
        }
    }

    #[inline]
    pub fn get_u64_data(&self, data: BymlData) -> Option<u64> {
        if data.node_type == NodeType::U64 {
            let value = u64::try_read(&self.data[data.as_offset()..], self.endian)
                .ok()?
                .0;
            Some(value)
        } else {
            None
        }
    }

    #[inline]
    pub fn get_double_data(&self, data: BymlData) -> Option<f64> {
        if data.node_type == NodeType::Double {
            let value = f64::try_read(&self.data[data.as_offset()..], self.endian)
                .ok()?
                .0;
            Some(value)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn basics() {
        let data =
            include_bytes!("../../test/byml/Mrg_01e57204_MrgD100_B4-B3-B2-1A90E17A.bcett.byml");
        let parser = super::BymlIter::new(data.as_slice()).unwrap();
        assert_eq!(parser.header().unwrap().root_node_offset, 264);
        assert_eq!(parser.root_node().unwrap(), super::BymlContainerHeader {
            len: 1,
            node_type: crate::byml::NodeType::Map,
        });
        for (k, v) in parser.iter_as_map().unwrap() {
            assert!(v.is_array());
            for v in parser.iter_array_data(v).unwrap() {
                assert!(v.is_map());
                for (k, v) in parser.iter_map_data(v).unwrap() {
                    let val = match v.node_type {
                        crate::byml::NodeType::String => parser.get_string_data(v).unwrap(),
                        _ => "other",
                    };
                    println!("{}: {}", k, val);
                }
            }
        }
    }
}
