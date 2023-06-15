use ::alloc::vec::Vec;
use num_traits::AsPrimitive;
use smartstring::alias::String;

use super::*;
use crate::{Error, Result};

/// A BYML hash node.
pub type Map = rustc_hash::FxHashMap<String, Byml>;
pub type HashMap = rustc_hash::FxHashMap<u32, Byml>;
pub type ValueHashMap = rustc_hash::FxHashMap<u32, (Byml, u32)>;

/// Represents a Nintendo binary YAML (BYML) document or node.
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum Byml {
    /// String value.
    String(String),
    /// Binary data (not used in BOTW).
    BinaryData(Vec<u8>),
    /// File data
    FileData(Vec<u8>),
    /// Array of BYML nodes.
    Array(Vec<Byml>),
    /// Hash map of BYML nodes with string keys.
    Map(Map),
    /// Hash map of BYML nodes with u32 keys.
    HashMap(HashMap),
    /// Hash map of BYML nodes with u32 keys and additional value.
    ValueHashMap(ValueHashMap),
    /// Boolean value.
    Bool(bool),
    /// 32-bit signed integer.
    I32(i32),
    /// 32-bit float.
    Float(f32),
    /// 32-bit unsigned integer.
    U32(u32),
    /// 64-bit signed integer.
    I64(i64),
    /// 64-bit unsigned integer.
    U64(u64),
    /// 64-bit float.
    Double(f64),
    /// Null value.
    Null,
}

impl Byml {
    #[inline]
    pub(super) fn get_node_type(&self) -> NodeType {
        match self {
            Byml::String(_) => NodeType::String,
            Byml::BinaryData(_) => NodeType::Binary,
            Byml::FileData(_) => NodeType::File,
            Byml::Array(_) => NodeType::Array,
            Byml::Map(_) => NodeType::Map,
            Byml::HashMap(_) => NodeType::HashMap,
            Byml::ValueHashMap(_) => NodeType::ValueHashMap,
            Byml::Bool(_) => NodeType::Bool,
            Byml::I32(_) => NodeType::I32,
            Byml::Float(_) => NodeType::Float,
            Byml::U32(_) => NodeType::U32,
            Byml::I64(_) => NodeType::I64,
            Byml::U64(_) => NodeType::U64,
            Byml::Double(_) => NodeType::Double,
            Byml::Null => NodeType::Null,
        }
    }

    #[inline(always)]
    pub(super) fn is_non_inline_type(&self) -> bool {
        matches!(
            self,
            Byml::Array(_)
                | Byml::Map(_)
                | Byml::HashMap(_)
                | Byml::ValueHashMap(_)
                | Byml::BinaryData(_)
                | Byml::FileData(_)
                | Byml::I64(_)
                | Byml::U64(_)
                | Byml::Double(_)
        )
    }

    fn type_name(&self) -> String {
        match self {
            Byml::String(_) => "String".into(),
            Byml::BinaryData(_) => "Binary".into(),
            Byml::FileData(_) => "File".into(),
            Byml::Array(_) => "Array".into(),
            Byml::Map(_) => "Map".into(),
            Byml::HashMap(_) => "HashMap".into(),
            Byml::ValueHashMap(_) => "ValueHashMap".into(),
            Byml::Bool(_) => "Bool".into(),
            Byml::I32(_) => "I32".into(),
            Byml::Float(_) => "Float".into(),
            Byml::U32(_) => "U32".into(),
            Byml::I64(_) => "I64".into(),
            Byml::U64(_) => "U64".into(),
            Byml::Double(_) => "Double".into(),
            Byml::Null => "Null".into(),
        }
    }

    /// Checks if the BYML node is a null node
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Get a reference to the inner bool value.
    pub fn as_bool(&self) -> Result<bool> {
        if let Self::Bool(v) = self {
            Ok(*v)
        } else {
            Err(Error::TypeError(self.type_name(), "Bool"))
        }
    }

    /// Get a reference to the inner i32 value.
    pub fn as_i32(&self) -> Result<i32> {
        if let Self::I32(v) = self {
            Ok(*v)
        } else {
            Err(Error::TypeError(self.type_name(), "I32"))
        }
    }

    /// Get a reference to the inner u32 value.
    pub fn as_u32(&self) -> Result<u32> {
        if let Self::U32(v) = self {
            Ok(*v)
        } else {
            Err(Error::TypeError(self.type_name(), "U32"))
        }
    }

    /// Get a reference to the inner i64 value.
    pub fn as_i64(&self) -> Result<i64> {
        if let Self::I64(v) = self {
            Ok(*v)
        } else {
            Err(Error::TypeError(self.type_name(), "I64"))
        }
    }

    /// Get a reference to the inner u64 value.
    pub fn as_u64(&self) -> Result<u64> {
        if let Self::U64(v) = self {
            Ok(*v)
        } else {
            Err(Error::TypeError(self.type_name(), "U64"))
        }
    }

    /// Get the inner value as an integer of any type. Casts the value using
    /// [`as`](https://doc.rust-lang.org/core/keyword.as.html) where necessary.
    /// Note that this is subject to all the normal risks of casting with `as`.
    pub fn as_int<T>(&self) -> Result<T>
    where
        T: Copy + 'static,
        i32: AsPrimitive<T>,
        u32: AsPrimitive<T>,
        i64: AsPrimitive<T>,
        u64: AsPrimitive<T>,
    {
        match self {
            Byml::I32(i) => Ok(i.as_()),
            Byml::I64(i) => Ok(i.as_()),
            Byml::U32(i) => Ok(i.as_()),
            Byml::U64(i) => Ok(i.as_()),
            _ => Err(Error::TypeError(self.type_name(), "an integer")),
        }
    }

    /// Get the inner value as a number of any type. Casts the value using
    /// [`as`](https://doc.rust-lang.org/core/keyword.as.html) where necessary.
    /// Note that this is subject to all the normal risks of casting with `as`.
    pub fn as_num<T>(&self) -> Result<T>
    where
        T: Copy + 'static,
        i32: AsPrimitive<T>,
        u32: AsPrimitive<T>,
        i64: AsPrimitive<T>,
        u64: AsPrimitive<T>,
        f32: AsPrimitive<T>,
        f64: AsPrimitive<T>,
    {
        match self {
            Byml::I32(i) => Ok(i.as_()),
            Byml::I64(i) => Ok(i.as_()),
            Byml::U32(i) => Ok(i.as_()),
            Byml::U64(i) => Ok(i.as_()),
            Byml::Float(i) => Ok(i.as_()),
            Byml::Double(i) => Ok(i.as_()),
            _ => Err(Error::TypeError(self.type_name(), "a number")),
        }
    }

    /// Get a reference to the inner f32 value.
    pub fn as_float(&self) -> Result<f32> {
        if let Self::Float(v) = self {
            Ok(*v)
        } else {
            Err(Error::TypeError(self.type_name(), "Float"))
        }
    }

    /// Get a reference to the inner f64 value.
    pub fn as_double(&self) -> Result<f64> {
        if let Self::Double(v) = self {
            Ok(*v)
        } else {
            Err(Error::TypeError(self.type_name(), "Double"))
        }
    }

    /// Get a reference to the inner string value.
    pub fn as_string(&self) -> Result<&String> {
        if let Self::String(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "String"))
        }
    }

    /// Get a reference to the inner byte slice.
    pub fn as_binary_data(&self) -> Result<&[u8]> {
        if let Self::BinaryData(v) = self {
            Ok(v.as_slice())
        } else {
            Err(Error::TypeError(self.type_name(), "BinaryData"))
        }
    }

    /// Get a reference to the inner array of BYML nodes.
    pub fn as_array(&self) -> Result<&[Byml]> {
        if let Self::Array(v) = self {
            Ok(v.as_slice())
        } else {
            Err(Error::TypeError(self.type_name(), "Array"))
        }
    }

    /// Get a reference to the inner string-keyed hash map of BYML nodes.
    pub fn as_map(&self) -> Result<&Map> {
        if let Self::Map(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Map"))
        }
    }

    /// Get a reference to the inner u32-keyed hash map of BYML nodes.
    pub fn as_hash_map(&self) -> Result<&HashMap> {
        if let Self::HashMap(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "HashMap"))
        }
    }

    /// Get a reference to the inner u32-keyed hash map of BYML nodes.
    pub fn as_value_hash_map(&self) -> Result<&ValueHashMap> {
        if let Self::ValueHashMap(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "ValueHashMap"))
        }
    }

    /// Get a mutable reference to the inner string value.
    pub fn as_mut_string(&mut self) -> Result<&mut String> {
        if let Self::String(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "String"))
        }
    }

    /// Get a mutable reference to the inner bool value.
    pub fn as_mut_bool(&mut self) -> Result<&mut bool> {
        if let Self::Bool(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Bool"))
        }
    }

    /// Get a mutable reference to the inner i32 value.
    pub fn as_mut_i32(&mut self) -> Result<&mut i32> {
        if let Self::I32(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "I32"))
        }
    }

    /// Get a mutable reference to the inner u32 value.
    pub fn as_mut_u32(&mut self) -> Result<&mut u32> {
        if let Self::U32(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "U32"))
        }
    }

    /// Get a mutable reference to the inner i64 value.
    pub fn as_mut_i64(&mut self) -> Result<&mut i64> {
        if let Self::I64(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "I64"))
        }
    }

    /// Get a mutable reference to the inner u64 value.
    pub fn as_mut_u64(&mut self) -> Result<&mut u64> {
        if let Self::U64(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "U64"))
        }
    }

    /// Get a mutable reference to the inner f32 value.
    pub fn as_mut_float(&mut self) -> Result<&mut f32> {
        if let Self::Float(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Float"))
        }
    }

    /// Get a mutable reference to the inner f64 value.
    pub fn as_mut_double(&mut self) -> Result<&mut f64> {
        if let Self::Double(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Double"))
        }
    }

    /// Get a mutable reference to the inner byte slice.
    pub fn as_mut_binary_data(&mut self) -> Result<&mut [u8]> {
        if let Self::BinaryData(v) = self {
            Ok(v.as_mut_slice())
        } else {
            Err(Error::TypeError(self.type_name(), "BinaryData"))
        }
    }

    /// Get a mutable reference to the inner array of BYML nodes.
    pub fn as_mut_array(&mut self) -> Result<&mut Vec<Byml>> {
        if let Self::Array(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Array"))
        }
    }

    /// Get a mutable reference to the inner hash map of BYML nodes.
    pub fn as_mut_map(&mut self) -> Result<&mut Map> {
        if let Self::Map(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Hash"))
        }
    }

    /// Get a reference to the inner u32-keyed hash map of BYML nodes.
    pub fn as_mut_hash_map(&mut self) -> Result<&mut HashMap> {
        if let Self::HashMap(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "HashMap"))
        }
    }

    /// Get a reference to the inner u32-keyed hash map of BYML nodes.
    pub fn as_mut_value_hash_map(&mut self) -> Result<&mut ValueHashMap> {
        if let Self::ValueHashMap(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "ValueHashMap"))
        }
    }

    /// Extract the inner string value.
    pub fn into_string(self) -> Result<String> {
        if let Self::String(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "String"))
        }
    }

    /// Extract the inner bool value.
    pub fn into_bool(self) -> Result<bool> {
        if let Self::Bool(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Bool"))
        }
    }

    /// Extract the inner i32 value.
    pub fn into_i32(self) -> Result<i32> {
        if let Self::I32(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "I32"))
        }
    }

    /// Extract the inner u32 value.
    pub fn into_u32(self) -> Result<u32> {
        if let Self::U32(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "U32"))
        }
    }

    /// Extract the inner i64 value.
    pub fn into_i64(self) -> Result<i64> {
        if let Self::I64(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "I64"))
        }
    }

    /// Extract the inner u64 value.
    pub fn into_u64(self) -> Result<u64> {
        if let Self::U64(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "U64"))
        }
    }

    /// Extract the inner f32 value.
    pub fn into_float(self) -> Result<f32> {
        if let Self::Float(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Float"))
        }
    }

    /// Extract the inner f64 value.
    pub fn into_double(self) -> Result<f64> {
        if let Self::Double(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Double"))
        }
    }

    /// Extract the inner byte slice value.
    pub fn into_binary_data(self) -> Result<Vec<u8>> {
        if let Self::BinaryData(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "BinaryData"))
        }
    }

    /// Extract the inner Byml array value.
    pub fn into_array(self) -> Result<Vec<Byml>> {
        if let Self::Array(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Array"))
        }
    }

    /// Extract the inner map value.
    pub fn into_map(self) -> Result<Map> {
        if let Self::Map(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "Map"))
        }
    }

    /// Extract the inner hash map value.
    pub fn into_hash_map(self) -> Result<HashMap> {
        if let Self::HashMap(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "HashMap"))
        }
    }

    /// Extract the inner value hash map value.
    pub fn into_value_hash_map(self) -> Result<ValueHashMap> {
        if let Self::ValueHashMap(v) = self {
            Ok(v)
        } else {
            Err(Error::TypeError(self.type_name(), "ValueHashMap"))
        }
    }
}

impl From<bool> for Byml {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl TryFrom<Byml> for bool {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        match value {
            Byml::Bool(v) => Ok(v),
            _ => Err(value),
        }
    }
}

impl From<i32> for Byml {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}

impl TryFrom<Byml> for i32 {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        value.as_int().map_err(|_| value)
    }
}

impl From<u32> for Byml {
    fn from(value: u32) -> Self {
        Self::U32(value)
    }
}

impl TryFrom<Byml> for u32 {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        value.as_int().map_err(|_| value)
    }
}

impl From<i64> for Byml {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl TryFrom<Byml> for i64 {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        value.as_int().map_err(|_| value)
    }
}

impl From<u64> for Byml {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}

impl TryFrom<Byml> for u64 {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        value.as_int().map_err(|_| value)
    }
}

impl From<f32> for Byml {
    fn from(value: f32) -> Self {
        Self::Float(value)
    }
}

impl TryFrom<Byml> for f32 {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        value.as_num().map_err(|_| value)
    }
}

impl From<f64> for Byml {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl TryFrom<Byml> for f64 {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        value.as_num().map_err(|_| value)
    }
}

impl TryFrom<Byml> for Vec<u8> {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        match value {
            Byml::BinaryData(v) => Ok(v),
            _ => Err(value),
        }
    }
}

impl From<Vec<Byml>> for Byml {
    fn from(value: Vec<Byml>) -> Self {
        Self::Array(value)
    }
}

impl TryFrom<Byml> for Vec<Byml> {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        match value {
            Byml::Array(v) => Ok(v),
            _ => Err(value),
        }
    }
}

impl From<Map> for Byml {
    fn from(value: Map) -> Self {
        Self::Map(value)
    }
}

impl TryFrom<Byml> for Map {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        match value {
            Byml::Map(v) => Ok(v),
            _ => Err(value),
        }
    }
}

impl From<HashMap> for Byml {
    fn from(value: HashMap) -> Self {
        Self::HashMap(value)
    }
}

impl TryFrom<Byml> for HashMap {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        match value {
            Byml::HashMap(map) => Ok(map),
            _ => Err(value),
        }
    }
}

impl From<ValueHashMap> for Byml {
    fn from(value: ValueHashMap) -> Self {
        Self::ValueHashMap(value)
    }
}

impl TryFrom<Byml> for ValueHashMap {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        match value {
            Byml::ValueHashMap(map) => Ok(map),
            _ => Err(value),
        }
    }
}

impl From<&str> for Byml {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

impl From<String> for Byml {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&String> for Byml {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

impl From<::alloc::string::String> for Byml {
    fn from(value: ::alloc::string::String) -> Self {
        Self::String(value.into())
    }
}

impl From<&::alloc::string::String> for Byml {
    fn from(value: &::alloc::string::String) -> Self {
        Self::String(value.into())
    }
}

impl TryFrom<Byml> for String {
    type Error = Byml;

    fn try_from(value: Byml) -> core::result::Result<Self, Self::Error> {
        match value {
            Byml::String(v) => Ok(v),
            _ => Err(value),
        }
    }
}

// impl From<&[u8]> for Byml {
//     fn from(value: &[u8]) -> Self {
//         Self::BinaryData(value.to_vec())
//     }
// }

impl From<&[Byml]> for Byml {
    fn from(value: &[Byml]) -> Self {
        Self::Array(value.to_vec())
    }
}

impl<S: Into<String>> FromIterator<(S, Byml)> for Byml {
    fn from_iter<T: IntoIterator<Item = (S, Byml)>>(iter: T) -> Self {
        Self::Map(iter.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }
}

impl FromIterator<Byml> for Byml {
    fn from_iter<T: IntoIterator<Item = Byml>>(iter: T) -> Self {
        Self::Array(iter.into_iter().collect())
    }
}

impl Default for Byml {
    fn default() -> Self {
        Self::Null
    }
}

impl PartialEq for Byml {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Byml::String(s1), Byml::String(s2)) => s1 == s2,
            (Byml::BinaryData(d1), Byml::BinaryData(d2)) => d1 == d2,
            (Byml::FileData(d1), Byml::FileData(d2)) => d1 == d2,
            (Byml::Array(a1), Byml::Array(a2)) => a1 == a2,
            (Byml::Map(h1), Byml::Map(h2)) => h1 == h2,
            (Byml::HashMap(h1), Byml::HashMap(h2)) => h1 == h2,
            (Byml::ValueHashMap(h1), Byml::ValueHashMap(h2)) => h1 == h2,
            (Byml::Bool(b1), Byml::Bool(b2)) => b1 == b2,
            (Byml::I32(i1), Byml::I32(i2)) => i1 == i2,
            (Byml::Float(f1), Byml::Float(f2)) => almost::equal(*f1, *f2),
            (Byml::U32(u1), Byml::U32(u2)) => u1 == u2,
            (Byml::I64(i1), Byml::I64(i2)) => i1 == i2,
            (Byml::U64(u1), Byml::U64(u2)) => u1 == u2,
            (Byml::Double(d1), Byml::Double(d2)) => almost::equal(*d1, *d2),
            (Byml::Null, Byml::Null) => true,
            _ => false,
        }
    }
}

impl PartialEq<Byml> for &Byml {
    fn eq(&self, other: &Byml) -> bool {
        self == other
    }
}

impl Eq for &Byml {}

impl core::hash::Hash for Byml {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        match self {
            Byml::String(s) => s.hash(state),
            Byml::BinaryData(b) => b.hash(state),
            Byml::FileData(b) => b.hash(state),
            Byml::Array(a) => a.hash(state),
            Byml::Map(h) => {
                for (k, v) in h.iter() {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Byml::HashMap(h) => {
                for (k, v) in h.iter() {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Byml::ValueHashMap(h) => {
                for (k, v) in h.iter() {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Byml::Bool(b) => b.hash(state),
            Byml::I32(i) => i.hash(state),
            Byml::Float(f) => {
                b"f".hash(state);
                f.to_bits().hash(state)
            }
            Byml::U32(u) => u.hash(state),
            Byml::I64(i) => i.hash(state),
            Byml::U64(u) => u.hash(state),
            Byml::Double(d) => {
                b"d".hash(state);
                d.to_bits().hash(state)
            }
            Byml::Null => core::hash::Hash::hash(&0, state),
        }
    }
}

impl<'a, I: Into<BymlIndex<'a>>> core::ops::Index<I> for Byml {
    type Output = Byml;

    fn index(&self, index: I) -> &Self::Output {
        match (self, index.into()) {
            (Byml::Array(a), BymlIndex::ArrayIdx(i)) => &a[i],
            (Byml::Map(h), BymlIndex::StringIdx(k)) => &h[k],
            (Byml::HashMap(h), BymlIndex::HashIdx(i)) => &h[&i],
            (Byml::ValueHashMap(h), BymlIndex::HashIdx(i)) => &h[&i].0,
            _ => panic!("Wrong index type or node type."),
        }
    }
}

impl<'a, I: Into<BymlIndex<'a>>> core::ops::IndexMut<I> for Byml {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        match (self, index.into()) {
            (Byml::Array(a), BymlIndex::ArrayIdx(i)) => &mut a[i],
            (Byml::Map(h), BymlIndex::StringIdx(k)) => h.get_mut(k).expect("Key should be in hash"),
            (Byml::HashMap(h), BymlIndex::HashIdx(i)) => {
                h.get_mut(&i).expect("Key should be in hash")
            }
            (Byml::ValueHashMap(h), BymlIndex::HashIdx(i)) => {
                &mut h.get_mut(&i).expect("Key should be in hash").0
            }
            _ => panic!("Wrong index type or node type."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors() {
        let mut actorinfo =
            Byml::from_binary(include_bytes!("../../test/byml/ActorInfo.product.byml")).unwrap();
        let actorinfo_hash = actorinfo.as_mut_map().unwrap();
        for obj in actorinfo_hash
            .get_mut("Actors")
            .unwrap()
            .as_mut_array()
            .unwrap()
        {
            let hash = obj.as_mut_map().unwrap();
            *hash.get_mut("name").unwrap().as_mut_string().unwrap() = "test".into();
            assert_eq!(hash["name"].as_string().unwrap(), "test");
        }
    }
}
