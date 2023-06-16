mod aglenv;
mod factory;

use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
    vec::Vec,
};
use core::{borrow::Borrow, hash::Hash, mem::size_of};

use byte::BytesExt;
use indexmap::IndexMap;
use num_integer::Integer;
use serde::Deserialize;

use super::*;
use crate::{util::FxHashMap, Endian, Result};
const HASH_MULTIPLIER: u32 = 0x65;

#[derive(Deserialize)]
#[allow(dead_code)]
struct AglEnvInfo {
    id: u16,
    i0: u16,
    ext: String,
    bext: String,
    s: Option<String>,
    align: i32,
    system: String,
    desc: String,
}

#[inline(always)]
fn align(pos: usize, alignment: usize) -> usize {
    let pos = pos as i64;
    let alignment = alignment as i64;
    (pos + (alignment - pos % alignment) % alignment) as usize
}

/// A simple SARC archive writer
#[derive(Clone)]
pub struct SarcWriter {
    pub endian: Endian,
    legacy: bool,
    hash_multiplier: u32,
    min_alignment: usize,
    alignment_map: FxHashMap<String, usize>,
    bin_endian: byte::ctx::Endian,
    /// Files to be written.
    pub files: IndexMap<String, Vec<u8>, core::hash::BuildHasherDefault<rustc_hash::FxHasher>>,
}

impl core::fmt::Debug for SarcWriter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SarcWriter")
            .field("endian", &self.endian)
            .field("legacy", &self.legacy)
            .field("hash_multiplier", &self.hash_multiplier)
            .field("min_alignment", &self.min_alignment)
            .field("alignment_map", &self.alignment_map)
            .field("files", &self.files.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl PartialEq for SarcWriter {
    fn eq(&self, other: &Self) -> bool {
        self.endian == other.endian
            && self.legacy == other.legacy
            && self.hash_multiplier == other.hash_multiplier
            && self.min_alignment == other.min_alignment
            && self.alignment_map == other.alignment_map
            && self.files == other.files
    }
}

impl Eq for SarcWriter {}

impl SarcWriter {
    /// A simple SARC archive writer
    pub fn new(endian: Endian) -> SarcWriter {
        SarcWriter {
            endian,
            legacy: false,
            hash_multiplier: HASH_MULTIPLIER,
            alignment_map: FxHashMap::default(),
            files: Default::default(),
            bin_endian: match endian {
                Endian::Big => byte::ctx::Endian::Big,
                Endian::Little => byte::ctx::Endian::Little,
            },
            min_alignment: 4,
        }
    }

    /// Creates a new SARC writer by taking attributes and files
    /// from an existing SARC reader
    pub fn from_sarc(sarc: &Sarc) -> SarcWriter {
        let endian = sarc.endian();
        SarcWriter {
            endian,
            legacy: false,
            hash_multiplier: HASH_MULTIPLIER,
            alignment_map: FxHashMap::default(),
            files: sarc
                .files()
                .filter_map(|f| f.name.map(|name| (name.to_string(), f.data.to_vec())))
                .collect(),
            bin_endian: endian.into(),
            min_alignment: sarc.guess_min_alignment(),
        }
    }

    /// Write a SARC archive to an in-memory buffer using the specified
    /// endianness. Default alignment requirements may be automatically
    /// added.
    pub fn to_binary(&mut self) -> Vec<u8> {
        let est_size: usize = self.est_size();
        let mut buf: Vec<u8> = alloc::vec![0u8; est_size];
        let written = self
            .write(&mut buf)
            .expect("SARC should write to memory without error");
        if written > buf.len() {
            panic!("Overflowed SARC buffer")
        } else {
            unsafe { buf.set_len(written) }
        }
        buf
    }

    #[inline]
    fn est_size(&self) -> usize {
        ((Sarc::MAGIC.len()
            + size_of::<ResHeader>()
            + ResFatHeader::MAGIC.len()
            + size_of::<ResFatHeader>()
            + ResFntHeader::MAGIC.len()
            + size_of::<ResFntHeader>()
            + self
                .files
                .iter()
                .map(|(n, d)| 0x10 + align(n.len() + 1, 4) + d.len())
                .sum::<usize>()) as f32
            * 1.5) as usize
    }

    /// Write a SARC archive to a Write + Seek writer using the specified
    /// endianness. Default alignment requirements may be automatically
    /// added.
    pub fn write<W: AsMut<[u8]>>(&mut self, mut buffer: W) -> Result<usize> {
        let buf = buffer.as_mut();
        if buf.len() < self.est_size() {
            Err(byte::Error::Incomplete.into())
        } else {
            let offset = &mut 0x14;
            buf.write_with(
                offset,
                ResFatHeader {
                    header_size: (ResFatHeader::MAGIC.len() + size_of::<ResFatHeader>()) as u16,
                    num_files: self.files.len() as u16,
                    hash_multiplier: self.hash_multiplier,
                },
                self.bin_endian,
            )?;

            self.files.sort_unstable_by(|ka, _, kb, _| {
                hash_name(HASH_MULTIPLIER, ka).cmp(&hash_name(HASH_MULTIPLIER, kb))
            });
            self.add_default_alignments();
            let mut alignments: Vec<usize> = Vec::with_capacity(self.files.len());
            {
                let mut rel_string_offset = 0;
                let mut rel_data_offset = 0;
                for (name, data) in self.files.iter() {
                    let alignment = self.get_alignment_for_file(name, data);
                    alignments.push(alignment);

                    let rel_offset = align(rel_data_offset, alignment);
                    buf.write_with(
                        offset,
                        ResFatEntry {
                            name_hash: hash_name(self.hash_multiplier, name.as_ref()),
                            rel_name_opt_offset: 1 << 24 | (rel_string_offset / 4),
                            data_begin: rel_offset as u32,
                            data_end: (rel_offset + data.len()) as u32,
                        },
                        self.bin_endian,
                    )?;

                    rel_data_offset = rel_offset + data.len();
                    rel_string_offset += align(name.len() + 1, 4) as u32;
                }
            }

            buf.write_with(
                offset,
                ResFntHeader {
                    header_size: 0x8,
                    reserved: 0,
                },
                self.bin_endian,
            )?;
            for (name, _) in self.files.iter() {
                buf.write_with(offset, name.as_bytes(), ())?;
                buf.write_with(offset, 0u8, self.bin_endian)?;
                *offset = align(*offset, 4);
            }

            let required_alignment = alignments
                .iter()
                .fold(1, |acc: usize, alignment| acc.lcm(alignment));
            *offset = align(*offset, required_alignment);
            let data_offset_begin = *offset as u32;
            for ((_, data), alignment) in self.files.iter().zip(alignments.iter()) {
                *offset = align(*offset, *alignment);
                buf.write_with(offset, data.as_slice(), ())?;
            }

            let file_size = *offset as u32;
            buf[..Sarc::MAGIC.len()].copy_from_slice(Sarc::MAGIC);
            *offset = Sarc::MAGIC.len();
            buf.write_with(
                offset,
                ResHeader {
                    header_size: (Sarc::MAGIC.len() + size_of::<ResHeader>()) as u16,
                    bom: self.endian,
                    file_size,
                    data_offset: data_offset_begin,
                    version: 0x0100,
                    reserved: 0,
                },
                (),
            )?;
            Ok(file_size as usize)
        }
    }

    /// Add or modify a data alignment requirement for a file type. Set the
    /// alignment to 1 to revert.
    ///
    /// # Arguments
    ///
    /// * `ext` - File extension without the dot (e.g. “bgparamlist”)
    /// * `alignment` - Data alignment (must be a power of 2)
    ///
    /// Panics if an invalid alignment is provided. If you're not passing an
    /// alignment that is known at compile-time, you should probably check
    /// using [`is_valid_alignment`] first.
    pub fn add_alignment_requirement(&mut self, ext: String, alignment: usize) {
        if !is_valid_alignment(alignment) {
            panic!("Invalid alignment requirement");
        }
        self.alignment_map.insert(ext, alignment);
    }

    /// Builder-style method to add or modify a data alignment requirement for
    /// a file type. Set the alignment to 1 to revert.
    ///
    /// # Arguments
    ///
    /// * `ext` - File extension without the dot (e.g. “bgparamlist”)
    /// * `alignment` - Data alignment (must be a power of 2)
    #[inline]
    pub fn with_alignment_requirement(mut self, ext: String, alignment: usize) -> Self {
        self.add_alignment_requirement(ext, alignment);
        self
    }

    fn add_default_alignments(&mut self) {
        for (ext, alignment) in aglenv::AGLENV_INFO {
            self.add_alignment_requirement(ext.to_string(), *alignment);
        }
        self.add_alignment_requirement("ksky".to_owned(), 8);
        self.add_alignment_requirement("bksky".to_owned(), 8);
        self.add_alignment_requirement("gtx".to_owned(), 0x2000);
        self.add_alignment_requirement("sharcb".to_owned(), 0x1000);
        self.add_alignment_requirement("sharc".to_owned(), 0x1000);
        self.add_alignment_requirement("baglmf".to_owned(), 0x80);
        self.add_alignment_requirement("bffnt".to_owned(), match self.endian {
            Endian::Big => 0x2000,
            Endian::Little => 0x1000,
        });
    }

    /// Set the minimum data alignment.
    ///
    /// Panics if an invalid alignment is provided. If you're not passing an
    /// alignment that is known at compile-time, you should probably check
    /// using [`is_valid_alignment`] first.
    pub fn set_min_alignment(&mut self, alignment: usize) {
        if !is_valid_alignment(alignment) {
            panic!("Invalid minimum SARC file alignment");
        }
        self.min_alignment = alignment;
    }

    /// Builder-style method to set the minimum data alignment
    #[inline]
    pub fn with_min_alignment(mut self, alignment: usize) -> Self {
        self.set_min_alignment(alignment);
        self
    }

    /// Set whether to use legacy mode (for games without a BOTW-style
    /// resource system) for addtional alignment restrictions
    #[inline]
    pub fn set_legacy_mode(&mut self, value: bool) {
        self.legacy = value
    }

    /// Builder-style method to set whether to use legacy mode (for games
    /// without a BOTW-style resource system) for addtional alignment
    /// restrictions
    #[inline]
    pub fn with_legacy_mode(mut self, value: bool) -> Self {
        self.set_legacy_mode(value);
        self
    }

    /// Set the endianness
    #[inline]
    pub fn set_endian(&mut self, endian: Endian) {
        self.endian = endian;
        self.bin_endian = endian.into();
    }

    /// Builder-style method to set the endianness
    #[inline]
    pub fn with_endian(mut self, endian: Endian) -> Self {
        self.set_endian(endian);
        self
    }

    /// Checks if a data slice represents a SARC archive
    pub fn is_file_sarc(data: &[u8]) -> bool {
        data.len() >= 0x20
            && (&data[0..4] == b"SARC" || (&data[0..4] == b"Yaz0" && &data[0x11..0x15] == b"SARC"))
    }

    fn get_alignment_for_new_binary_file(data: &[u8]) -> usize {
        if data.len() <= 0x20 {
            return 1;
        }
        let offset = &mut 0xC;
        if let Ok(endian) = data.read_with::<Endian>(offset, ()) {
            *offset = 0x1C;
            let file_size: u32 = match endian {
                Endian::Big => {
                    data.read_with(offset, byte::BE)
                        .expect("Should fine valid u32 file size")
                }
                Endian::Little => {
                    data.read_with(offset, byte::LE)
                        .expect("Should fine valid u32 file size")
                }
            };
            if file_size as usize != data.len() {
                return 1;
            } else {
                return 1 << data[0xE];
            }
        }
        1
    }

    fn get_alignment_for_cafe_bflim(data: &[u8]) -> usize {
        if data.len() <= 0x28 || &data[data.len() - 0x28..data.len() - 0x24] != b"FLIM" {
            1
        } else {
            let alignment: u16 = data
                .read_with(&mut (data.len() - 0x8), byte::BE)
                .expect("BFLIM should have u16 alignment info");
            alignment as usize
        }
    }

    fn get_alignment_for_file(&self, name: impl AsRef<str>, data: &[u8]) -> usize {
        let name = name.as_ref();
        let ext = match name.rfind('.') {
            Some(idx) => &name[idx + 1..],
            None => "",
        };
        let mut alignment = self.min_alignment;
        if let Some(requirement) = self.alignment_map.get(ext) {
            alignment = alignment.lcm(requirement);
        }
        if self.legacy && Self::is_file_sarc(data) {
            alignment = alignment.lcm(&0x2000);
        }
        if self.legacy || !factory::FACTORY_NAMES.contains(&ext) {
            alignment = alignment.lcm(&Self::get_alignment_for_new_binary_file(data));
            if let Endian::Big = self.endian {
                alignment = alignment.lcm(&Self::get_alignment_for_cafe_bflim(data));
            }
        }
        alignment
    }

    /// Add a file to the archive, with greater generic flexibility than using
    /// `insert` on the `files` field.
    #[inline]
    pub fn add_file(&mut self, name: impl Into<String>, data: impl Into<Vec<u8>>) {
        self.files.insert(name.into(), data.into());
    }

    /// Builder-style method to add a file to the archive.
    #[inline]
    pub fn with_file(mut self, name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        self.add_file(name, data);
        self
    }

    /// Add files to the archive from an iterator, with greater generic
    /// flexibility than using `extend` on the `files` field.
    #[inline]
    pub fn add_files<N, D>(&mut self, iter: impl IntoIterator<Item = (N, D)>)
    where
        N: Into<String>,
        D: Into<Vec<u8>>,
    {
        self.files.extend(
            iter.into_iter()
                .map(|(name, data)| (name.into(), data.into())),
        );
    }

    /// Builder-style method to add files to the archive from an iterator.
    #[inline]
    pub fn with_files<N, D>(mut self, iter: impl IntoIterator<Item = (N, D)>) -> Self
    where
        N: Into<String>,
        D: Into<Vec<u8>>,
    {
        self.add_files(iter);
        self
    }

    /// Remove a file from the archive, for convenience.
    #[inline]
    pub fn remove_file<Q: ?Sized + Hash + Eq>(&mut self, name: &Q)
    where
        String: Borrow<Q>,
    {
        self.files.remove(name);
    }

    /// Get a file's data from the archive, for convience.
    #[inline]
    pub fn get_file<Q: ?Sized + Hash + Eq>(&mut self, name: &Q) -> Option<&Vec<u8>>
    where
        String: Borrow<Q>,
    {
        self.files.get(name)
    }
}

impl From<&Sarc<'_>> for SarcWriter {
    fn from(sarc: &Sarc) -> Self {
        Self::from_sarc(sarc)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use crate::sarc::{Sarc, SarcWriter};

    #[test]
    fn make_sarc() {
        for file in [
            "ActorObserverByActorTagTag.sarc",
            "test.sarc",
            "A-1.00.sarc",
            "Common.blarc",
        ] {
            let data = std::fs::read(std::path::Path::new("test/sarc").join(file)).unwrap();
            let sarc = Sarc::new(&data).unwrap();
            let mut sarc_writer = SarcWriter::from_sarc(&sarc);
            sarc_writer.remove_file("Bob");
            let new_data = sarc_writer.to_binary();
            let new_sarc = Sarc::new(&new_data).unwrap();
            if !Sarc::are_files_equal(&sarc, &new_sarc) {
                for (f1, f2) in sarc.files().zip(new_sarc.files()) {
                    if f1 != f2 {
                        std::fs::write("test/f1", f1.data).unwrap();
                        std::fs::write("test/f2", f2.data).unwrap();
                        panic!("File {:?} has changed in SARC {:?}", f1.name, file);
                    }
                }
            }
            if data != new_data {
                dbg!(sarc);
                dbg!(new_sarc);
                panic!(
                    "Roundtrip not binary identical, wrong byte at offset {}",
                    data.iter()
                        .zip(new_data.iter())
                        .enumerate()
                        .find(|(_, (b1, b2))| *b1 != *b2)
                        .unwrap()
                        .0
                );
            }
        }
    }
}
