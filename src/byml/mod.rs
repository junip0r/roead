//! Port of the `oead::byml` module.
//!
//! A `Byml` type will usually be constructed from binary data or a YAML string,
//! e.g.
//! ```
//! # use roead::byml::Byml;
//! # use std::{fs::read, error::Error};
//! # fn docttest() -> Result<(), Box<dyn Error>> {
//! let buf: Vec<u8> = std::fs::read("test/byml/A-1_Dynamic.byml")?;
//! let map_unit = Byml::from_binary(&buf)?;
//! let text: String = std::fs::read_to_string("test/byml/A-1_Dynamic.yml")?;
//! //let map_unit2 = Byml::from_text(&text)?;
//! //assert_eq!(map_unit, map_unit2);
//! # Ok(())
//! # }
//! ```
//! You can also easily serialize to binary or a YAML string.
//! ```no_run
//! # use roead::{byml::Byml, Endian};
//! # fn docttest() -> Result<(), Box<dyn std::error::Error>> {
//! let buf: Vec<u8> = std::fs::read("test/aamp/A-1_Dynamic.byml")?;
//! let map_unit = Byml::from_binary(&buf)?;
//! //std::fs::write("A-1_Static.yml", &map_unit.to_text())?;
//! std::fs::write(
//!     "test/aamp/A-1_Dynamic.byml",
//!     &map_unit.to_binary(Endian::Big),
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! A number of convenience getters are available which return a result for a
//! variant value:
//! ```
//! # use roead::byml::Byml;
//! # fn docttest() -> Result<(), Box<dyn std::error::Error>> {
//! # let some_data = b"BYML";
//! let doc = Byml::from_binary(some_data)?;
//! let map = doc.as_map().unwrap();
//! # Ok(())
//! # }
//! ```
//!
//! Most of the node types are fairly self-explanatory. Arrays are implemented
//! as `Vec<Byml>`, and maps as `FxHashMap<String, Byml>`. The new v7 hash maps
//! are `FxHashMap<u32, Byml>` and `FxHashMap<u32, (Byml, u32)>`.
//!
//! For convenience, a `Byml` *known* to be an array or map can be
//! indexed. **Panics if the node has the wrong type, the index has the wrong
//! type, or the index is not found**.
//! ```
//! # use roead::byml::Byml;
//! # fn docttest() -> Result<(), Box<dyn std::error::Error>> {
//! let buf: Vec<u8> = std::fs::read("test/byml/ActorInfo.product.byml")?;
//! let actor_info = Byml::from_binary(&buf)?;
//! assert_eq!(actor_info["Actors"].as_array().unwrap().len(), 7934);
//! assert_eq!(actor_info["Hashes"][0].as_i32().unwrap(), 31119);
//! # Ok(())
//! # }
//! ```
#[cfg(feature = "alloc")]
mod alloc;
mod parser;
#[cfg(feature = "yaml")]
mod text;
#[cfg(feature = "alloc")]
mod writer;
#[cfg(not(feature = "alloc"))]
pub use parser::BymlIter;
use smartstring::alias::String;

#[cfg(feature = "alloc")]
pub use self::alloc::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[binrw::binrw]
#[brw(repr = u8)]
#[repr(u8)]
pub enum NodeType {
    HashMap = 0x20,
    ValueHashMap = 0x21,
    String = 0xa0,
    Binary = 0xa1,
    File = 0xa2,
    Array = 0xc0,
    Map = 0xc1,
    StringTable = 0xc2,
    Bool = 0xd0,
    I32 = 0xd1,
    Float = 0xd2,
    U32 = 0xd3,
    I64 = 0xd4,
    U64 = 0xd5,
    Double = 0xd6,
    Null = 0xff,
}

#[inline(always)]
const fn is_container_type(node_type: NodeType) -> bool {
    matches!(
        node_type,
        NodeType::Array | NodeType::Map | NodeType::ValueHashMap | NodeType::HashMap
    )
}

#[inline(always)]
const fn is_valid_version(version: u16) -> bool {
    version >= 1 && version < 8
}

#[derive(Debug, thiserror_no_std::Error)]
pub enum BymlError {
    #[error("Invalid version: {0}")]
    InvalidVersion(u16),
    #[cfg(feature = "alloc")]
    #[error("Incorrect BYML node type: found `{0}`, expected `{1}`.")]
    TypeError(::alloc::string::String, ::alloc::string::String),
    #[error(transparent)]
    BinaryRwError(#[from] binrw::Error),
    #[cfg(feature = "binrw")]
    #[error(transparent)]
    IoError(#[from] binrw::io::Error),
    #[error("Error parsing BYML data: {0}")]
    ParseError(&'static str),
}

/// Convenience type used for indexing into `Byml`s
pub enum BymlIndex<'a> {
    /// Index into a hash node. The key is a string.
    StringIdx(&'a str),
    /// Index into a hash node. The key is a u32 hash.
    HashIdx(u32),
    /// Index into an array node. The index is an integer.
    ArrayIdx(usize),
}

impl<'a> From<&'a str> for BymlIndex<'a> {
    fn from(s: &'a str) -> Self {
        Self::StringIdx(s)
    }
}

impl<'a> From<&'a String> for BymlIndex<'a> {
    fn from(s: &'a String) -> Self {
        Self::StringIdx(s)
    }
}

impl<'a> From<usize> for BymlIndex<'a> {
    fn from(idx: usize) -> Self {
        Self::ArrayIdx(idx)
    }
}

impl<'a> From<i32> for BymlIndex<'a> {
    fn from(value: i32) -> Self {
        assert!(!value.is_negative());
        Self::ArrayIdx(value as usize)
    }
}

impl<'a> From<u32> for BymlIndex<'a> {
    fn from(value: u32) -> Self {
        Self::HashIdx(value)
    }
}

#[cfg(test)]
pub(self) static FILES: &[&str] = &[
    "A-1_Dynamic",
    #[cfg(feature = "yaz0")]
    "D-3_Dynamic",
    "EventInfo.product",
    "GameROMPlayer",
    "LevelSensor",
    "MainFieldLocation",
    "MainFieldStatic",
    "Preset0_Field",
    "ActorInfo.product",
    "ElectricGenerator.Nin_NX_NVN.esetb",
    "USen",
    "Mrg_01e57204_MrgD100_B4-B3-B2-1A90E17A.bcett",
];
