//! # roead
//!
//! [![crates.io](https://img.shields.io/crates/v/roead)](https://crates.io/crates/roead)
//! [![api](https://img.shields.io/badge/api-rustdoc-558b2f)](https://nicenenerd.github.io/roead/roead/)
//! [![license](https://img.shields.io/badge/license-GPL-blue)](https://spdx.org/licenses/GPL-3.0-or-later.html)
//! [![build](https://img.shields.io/github/actions/workflow/status/NiceneNerd/roead/test.yml)](https://github.com/NiceneNerd/roead/actions/workflows/test.yml)
//!
//! ## A Rusty child of the oead C++ library
//! **oead** is a C++ library for common file formats that are used in modern
//! first-party Nintendo EAD (now EPD) titles.
//!
//! Currently, oead only handles very common formats that are extensively used
//! in recent games such as *Breath of the Wild* and *Super Mario Odyssey*.
//!
//! * [AAMP](https://zeldamods.org/wiki/AAMP) (binary parameter archive): Only version 2 is
//!   supported.
//! * [BYML](https://zeldamods.org/wiki/BYML) (binary YAML): Versions 2, 3, and 4 are supported.
//! * [SARC](https://zeldamods.org/wiki/SARC) (archive)
//! * [Yaz0](https://zeldamods.org/wiki/Yaz0) (compression algorithm)
//!
//! The roead project brings oead's core functionality, by directly porting or
//! (for the yaz0 module) providing safe and idiomatic bindings to oead's
//! features. (The Grezzo datasheets are not supported.) For more info on oead
//! itself, visit [its GitHub repo](https://github.com/zeldamods/oead/).
//!
//! Each of roead's major modules is configurable as a feature. The default
//! feature set includes `byml`, `aamp`, `sarc,` and `yaz0`. For compatibility
//! with many existing tools for these formats, there is also a `yaml` feature
//! which enables serializing/deserializing AAMP and BYML files as YAML
//! documents. Finally, serde support is available using the `with-serde`
//! feature.
//!
//! For API documentation, see the docs for each module.
//!
//! ## Building from Source
//!
//! Most of roead is pure Rust and can compiled with any relatively recent
//! *nightly* release. However, the yaz0 module provides FFI bindings to oead
//! code, so to use it the following additional requirements are necessary:
//!
//! - CMake 3.12+
//! - A compiler that supports C++17
//! - Everything necessary to build zlib
//!
//! First, clone the repository, then enter the roead directory and run
//! `git submodule update --init --recursive`.
//!
//! ## Contributing
//!
//! Issue tracker: <https://github.com/NiceneNerd/roead/issues>  
//! Source code: <https://github.com/NiceneNerd/roead>
//!
//! This project is licensed under the GPLv3+ license. oead is licensed under
//! the GPLv2+ license.
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "aamp")]
pub mod aamp;
#[cfg(feature = "byml")]
pub mod byml;
#[cfg(feature = "sarc")]
pub mod sarc;
#[cfg(feature = "types")]
pub mod types;
mod util;
#[cfg(feature = "yaml")]
mod yaml;
#[cfg(feature = "yaz0")]
pub mod yaz0;

/// Error type for this crate.
#[derive(Debug, thiserror_no_std::Error)]

pub enum Error {
    #[cfg(feature = "alloc")]
    #[error("Bad magic value: found `{0}`, expected `{1}`.")]
    BadMagic(alloc::string::String, &'static str),
    #[cfg(not(feature = "alloc"))]
    #[error("Bad magic value: found `{0:?}`, expected `{1}`.")]
    BadMagic([u8; 4], &'static str),
    #[error("Data too short: found {0:#x} bytes, expected >= {1:#x}.")]
    InsufficientData(usize, usize),
    #[error("{0}")]
    InvalidData(&'static str),
    #[cfg(feature = "alloc")]
    #[error("{0}")]
    InvalidDataD(alloc::string::String),
    #[error("Found {0}, expected {1}")]
    TypeError(smartstring::alias::String, &'static str),
    #[cfg(all(feature = "no_std_io", not(feature = "std")))]
    #[error(transparent)]
    Io(#[from] no_std_io::io::Error),
    #[cfg(all(feature = "binrw", not(feature = "std")))]
    #[error(transparent)]
    BinIo(#[from] binrw::io::Error),
    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(feature = "binrw")]
    #[error(transparent)]
    BinarySerde(#[from] binrw::Error),
    #[cfg(feature = "byte")]
    #[error("Binary error: {}", display_byte_error(.0))]
    BinarySerDe(byte::Error),
    #[error(transparent)]
    InvalidUtf8(#[from] core::str::Utf8Error),
    #[cfg(feature = "yaml")]
    #[error(transparent)]
    InvalidNumber(#[from] lexical::Error),
    #[cfg(feature = "yaml")]
    #[error("Parsing YAML failed: {0}")]
    InvalidYaml(#[from] ryml::Error),
    #[cfg(feature = "yaml")]
    #[error("Parsing YAML binary data failed: {0}")]
    InvalidYamlBinary(#[from] base64::DecodeError),
    #[cfg(feature = "yaz0")]
    #[error(transparent)]
    Yaz0Error(#[from] cxx::Exception),
    #[cfg(feature = "alloc")]
    #[error("{0}")]
    Any(alloc::string::String),
}

#[cfg(feature = "byte")]
impl From<byte::Error> for Error {
    fn from(err: byte::Error) -> Self {
        Self::BinarySerDe(err)
    }
}

#[cfg(all(feature = "byte", feature = "alloc"))]
fn display_byte_error(error: &byte::Error) -> alloc::string::String {
    #[cfg(not(feature = "std"))]
    use alloc::borrow::ToOwned;

    match error {
        byte::Error::Incomplete => "Insufficient data".to_owned(),
        byte::Error::BadOffset(off) => alloc::format!("Invalid offset: {off}"),
        byte::Error::BadInput { err } => (*err).to_owned(),
    }
}

#[cfg(all(feature = "byte", not(feature = "alloc")))]
fn display_byte_error(error: &byte::Error) -> &str {
    match error {
        byte::Error::Incomplete => "Insufficient data",
        byte::Error::BadOffset(_) => "Invalid offset",
        byte::Error::BadInput { err } => err,
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[repr(u16)]
/// Represents endianness where applicable.
///
/// Generally in the game ROM, big endian is used for Wii U and little endian
/// is used for Switch.
pub enum Endian {
    /// Big Endian (Wii U)
    Big = 0xFFFE,
    /// Little Endian (Switch)
    Little = 0xFEFF,
}

#[cfg(feature = "byte")]
impl From<byte::ctx::Endian> for Endian {
    fn from(value: byte::ctx::Endian) -> Self {
        match value {
            byte::ctx::Endian::Big => Self::Big,
            byte::ctx::Endian::Little => Self::Little,
        }
    }
}

#[cfg(feature = "byte")]
impl From<Endian> for byte::ctx::Endian {
    fn from(value: Endian) -> Self {
        match value {
            Endian::Big => Self::Big,
            Endian::Little => Self::Little,
        }
    }
}

#[cfg(feature = "byte")]
impl byte::TryRead<'_> for Endian {
    fn try_read(bytes: &'_ [u8], _ctx: ()) -> byte::Result<(Self, usize)> {
        const LEN: usize = core::mem::size_of::<Endian>();
        if bytes.len() < LEN {
            Err(byte::Error::Incomplete)
        } else {
            match &bytes[..2] {
                b"\xfe\xff" => Ok((Self::Big, LEN)),
                b"\xff\xfe" => Ok((Self::Little, LEN)),
                _ => Err(byte::Error::BadInput { err: "Invalid BOM" }),
            }
        }
    }
}

#[cfg(feature = "byte")]
impl byte::TryWrite for Endian {
    fn try_write(self, bytes: &mut [u8], _ctx: ()) -> byte::Result<usize> {
        const LEN: usize = core::mem::size_of::<Endian>();
        if bytes.len() < LEN {
            Err(byte::Error::Incomplete)
        } else {
            match self {
                Self::Big => bytes[..2].copy_from_slice(b"\xfe\xff"),
                Self::Little => bytes[..2].copy_from_slice(b"\xff\xfe"),
            };
            Ok(LEN)
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

impl Clone for Error {
    fn clone(&self) -> Self {
        todo!()
    }
}
