// use alloc::{borrow::Cow, string::ToString};
use core::{
    hash::{Hash, Hasher},
    mem::size_of,
};

use byte::{BytesExt, TryRead};
#[cfg(feature = "alloc")]
use join_str::jstr;
use num_integer::Integer;

use super::*;
use crate::{Error, Result};

fn find_null(data: &[u8]) -> Result<usize> {
    data.iter()
        .position(|b| b == &0u8)
        .ok_or(Error::InvalidData(
            "SARC filename contains unterminated string",
        ))
}

/// Iterator over [`File`] entries in a [`Sarc`].
#[derive(Debug)]
pub struct FileIterator<'a> {
    sarc: &'a Sarc<'a>,
    index: usize,
    entry_offset: usize,
    entry: ResFatEntry,
}

impl<'a> Iterator for FileIterator<'a> {
    type Item = File<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.sarc.num_files as usize {
            None
        } else {
            self.entry_offset =
                self.sarc.entries_offset as usize + size_of::<ResFatEntry>() * self.index;
            self.entry = ResFatEntry::try_read(
                &self.sarc.data[self.entry_offset..],
                self.sarc.endian.into(),
            )
            .map(|(v, _)| v)
            .ok()?;
            self.index += 1;
            Some(File {
                name:  if self.entry.rel_name_opt_offset != 0 {
                    let name_offset = self.sarc.names_offset as usize
                        + (self.entry.rel_name_opt_offset & 0xFFFFFF) as usize * 4;
                    let term_pos = find_null(&self.sarc.data[name_offset..]).ok()?;
                    Some(
                        core::str::from_utf8(&self.sarc.data[name_offset..name_offset + term_pos])
                            .ok()?,
                    )
                } else {
                    None
                },
                data:  self.sarc.data.get(
                    (self.sarc.data_offset + self.entry.data_begin) as usize
                        ..(self.sarc.data_offset + self.entry.data_end) as usize,
                )?,
                index: self.index,
                sarc:  self.sarc,
            })
        }
    }
}

#[cfg(feature = "alloc")]
type Buffer<'a> = alloc::borrow::Cow<'a, [u8]>;
#[cfg(not(feature = "alloc"))]
type Buffer<'a> = &'a [u8];

#[derive(Clone)]
/// A simple SARC archive reader
pub struct Sarc<'a> {
    num_files: u16,
    entries_offset: u16,
    hash_multiplier: u32,
    data_offset: u32,
    names_offset: u32,
    endian: Endian,
    data: Buffer<'a>,
}

impl core::fmt::Debug for Sarc<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sarc")
            .field("num_files", &self.num_files)
            .field("entries_offset", &self.entries_offset)
            .field("hash_multiplier", &self.hash_multiplier)
            .field("data_offset", &self.data_offset)
            .field("names_offset", &self.names_offset)
            .field("endian", &self.endian)
            .finish()
    }
}

impl PartialEq for Sarc<'_> {
    /// Returns true if and only if the raw archive data is identical
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for Sarc<'_> {}

impl Hash for Sarc<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state)
    }
}

impl<'a, S: core::borrow::Borrow<str>> core::ops::Index<S> for Sarc<'a> {
    type Output = [u8];

    fn index(&self, index: S) -> &Self::Output {
        self.get_data(index.borrow())
            .expect("File not found in SARC")
    }
}

impl<'a> Sarc<'_> {
    pub(crate) const MAGIC: &[u8] = b"SARC";

    /// Parses a SARC archive from binary data.
    ///
    /// **Note**: If and only if the `yaz0` feature is enabled, this function
    /// automatically decompresses the SARC when necessary.
    pub fn new<T: Into<Buffer<'a>>>(data: T) -> crate::Result<Sarc<'a>> {
        #[allow(unused_mut)]
        let mut data = data.into();

        #[cfg(feature = "yaz0")]
        {
            if data.starts_with(b"Yaz0") {
                data = crate::yaz0::decompress(&data)?.into();
            }
        }

        if data.len() < 0x40 {
            return Err(Error::InsufficientData(data.len(), 0x40));
        }
        if &data[..Self::MAGIC.len()] != Self::MAGIC {
            #[cfg(feature = "alloc")]
            return Err(Error::BadMagic(
                alloc::string::String::from_utf8_lossy(&data[..Self::MAGIC.len()]).to_string(),
                "SARC",
            ));
            #[cfg(not(feature = "alloc"))]
            return Err(Error::BadMagic(data[..4].try_into().unwrap(), "SARC"));
        }
        let offset = &mut Self::MAGIC.len();

        let header: ResHeader = data.read_with(offset, ())?;
        if header.version != 0x0100 {
            return Err(Error::InvalidData("Invalid SARC version (expected 0x100)"));
        }
        if header.header_size as usize != 0x14 {
            return Err(Error::InvalidData("SARC header wrong size (expected 0x14)"));
        }
        let endian: byte::ctx::Endian = header.bom.into();

        let fat_header: ResFatHeader = data.read_with(offset, endian)?;
        if fat_header.header_size as usize != 0x0C {
            return Err(Error::InvalidData("SFAT header wrong size (expected 0x0C)"));
        }
        if (fat_header.num_files >> 0xE) != 0 {
            #[cfg(feature = "alloc")]
            return Err(Error::InvalidDataD(jstr!(
                "Too many files in SARC ({&fat_header.num_files.to_string()})"
            )));
            #[cfg(not(feature = "alloc"))]
            return Err(Error::InvalidData("Too many files in SARC"));
        }

        let num_files = fat_header.num_files;
        let entries_offset = *offset as u16;
        let hash_multiplier = fat_header.hash_multiplier;
        let data_offset = header.data_offset;

        let fnt_header_offset = entries_offset as usize + 0x10 * num_files as usize;
        *offset = fnt_header_offset;
        let fnt_header: ResFntHeader = data.read_with(offset, endian)?;
        if fnt_header.header_size as usize != 0x08 {
            return Err(Error::InvalidData("SFNT header wrong size (expected 0x8)"));
        }

        let names_offset = *offset as u32;
        if data_offset < names_offset {
            return Err(Error::InvalidData("Invalid name table offset in SARC"));
        }
        Ok(Sarc {
            data,
            data_offset,
            endian: header.bom,
            entries_offset,
            num_files,
            hash_multiplier,
            names_offset,
        })
    }

    /// Get the number of files that are stored in the archive
    pub fn len(&self) -> usize {
        self.num_files as usize
    }

    /// Check if the SARC contains no files.
    pub fn is_empty(&self) -> bool {
        self.num_files == 0
    }

    /// Get the offset to the beginning of file data
    pub fn data_offset(&self) -> usize {
        self.data_offset as usize
    }

    /// Get the archive endianness
    pub fn endian(&self) -> Endian {
        self.endian
    }

    #[inline(always)]
    fn find_file(&self, file: &str) -> Result<Option<usize>> {
        if self.num_files == 0 {
            return Ok(None);
        }
        let needle_hash = hash_name(self.hash_multiplier, file);
        let mut a: u32 = 0;
        let mut b: u32 = self.num_files as u32 - 1;
        while a <= b {
            let m: u32 = (a + b) / 2;
            let offset = &mut (self.entries_offset as usize + 0x10 * m as usize);
            let hash: u32 = self.data.read_with(offset, self.endian.into())?;
            match needle_hash.cmp(&hash) {
                core::cmp::Ordering::Less => {
                    match m.checked_sub(1) {
                        Some(v) => b = v,
                        None => return Ok(None),
                    }
                }
                core::cmp::Ordering::Greater => a = m + 1,
                core::cmp::Ordering::Equal => return Ok(Some(m as usize)),
            }
        }
        Ok(None)
    }

    /// Get a file by name, returning `None` on its absence or any error.
    /// If you need to know the error, use [`Sarc::try_get`].
    pub fn get(&self, file: &str) -> Option<File> {
        let file_index = self.find_file(file).ok()?;
        file_index.and_then(|i| self.file_at(i).ok())
    }

    /// Get a file by name, returning a [`Result`] of an [`Option`]. This
    /// distinguishes between failed parsing (e.g. due to a corrupted SARC)
    /// and the absence of the file. If you don't care about any potential
    /// errors, just whether you can get the file data, use [`Sarc::get`].
    pub fn try_get(&self, file: &str) -> Result<Option<File>> {
        let file_index = self.find_file(file)?;
        file_index.map(|i| self.file_at(i)).transpose()
    }

    /// Get file data by name, returning a [`Result`] of an [`Option`]. This
    /// distinguishes between failed parsing (e.g. due to a corrupted SARC)
    /// and the absence of the file. If you don't care about any potential
    /// errors, just whether you can get the file data, use [`Sarc::get_data`].
    pub fn try_get_data(&self, file: &str) -> Result<Option<&[u8]>> {
        let file_index = self.find_file(file)?;
        file_index
            .map(|i| -> Result<&[u8]> {
                let entry_offset = self.entries_offset as usize + size_of::<ResFatEntry>() * i;
                let (entry, _) =
                    ResFatEntry::try_read(&self.data[entry_offset..], self.endian.into())?;
                Ok(&self.data[(self.data_offset + entry.data_begin) as usize
                    ..(self.data_offset + entry.data_end) as usize])
            })
            .transpose()
    }

    /// Get file data by name, returning `None` on its absence or any error.
    /// If you need to know the error, use [`Sarc::try_get_data`].
    pub fn get_data(&self, file: &str) -> Option<&[u8]> {
        self.try_get_data(file).ok().flatten()
    }

    /// Get a file by index. Returns error if index > file count.
    pub fn file_at(&self, index: usize) -> Result<File> {
        if index >= self.num_files as usize {
            #[cfg(feature = "alloc")]
            return Err(Error::InvalidDataD(jstr!(
                "No file in SARC at index {&index.to_string()}"
            )));
            #[cfg(not(feature = "alloc"))]
            return Err(Error::InvalidData("SARC file index out of bounds"));
        }

        let entry_offset = self.entries_offset as usize + size_of::<ResFatEntry>() * index;
        let (entry, _) = ResFatEntry::try_read(&self.data[entry_offset..], self.endian.into())?;

        Ok(File {
            name: if entry.rel_name_opt_offset != 0 {
                let name_offset = self.names_offset as usize
                    + (entry.rel_name_opt_offset & 0xFFFFFF) as usize * 4;
                let term_pos = find_null(&self.data[name_offset..])?;
                Some(core::str::from_utf8(
                    &self.data[name_offset..name_offset + term_pos],
                )?)
            } else {
                None
            },
            data: &self.data[(self.data_offset + entry.data_begin) as usize
                ..(self.data_offset + entry.data_end) as usize],
            index,
            sarc: self,
        })
    }

    /// Returns an iterator over the contained files
    pub fn files(&self) -> FileIterator<'_> {
        FileIterator {
            entry: ResFatEntry {
                name_hash: 0,
                rel_name_opt_offset: 0,
                data_begin: 0,
                data_end: 0,
            },
            index: 0,
            entry_offset: self.entries_offset as usize,
            sarc: self,
        }
    }

    /// Guess the minimum data alignment for files that are stored in the
    /// archive
    pub fn guess_min_alignment(&self) -> usize {
        const MIN_ALIGNMENT: u32 = 4;
        let mut gcd = MIN_ALIGNMENT;
        for _ in 0..self.num_files {
            let (entry, _) = ResFatEntry::try_read(
                &self.data[self.entries_offset as usize..],
                self.endian.into(),
            )
            .expect("Data should have valid ResFatEntry");
            gcd = gcd.gcd(&(self.data_offset + entry.data_begin));
        }

        if !is_valid_alignment(gcd as usize) {
            return MIN_ALIGNMENT as usize;
        }
        gcd as usize
    }

    /// Returns true is each archive contains the same files
    pub fn are_files_equal(sarc1: &Sarc, sarc2: &Sarc) -> bool {
        if sarc1.len() != sarc2.len() {
            return false;
        }

        for (file1, file2) in sarc1.files().zip(sarc2.files()) {
            if file1 != file2 {
                return false;
            }
        }
        true
    }
}

#[cfg(all(test))]
mod tests {
    use super::*;
    #[test]
    fn parse_sarc() {
        let data = include_bytes!("../../test/sarc/Dungeon119.pack");
        let sarc = Sarc::new(data.as_slice()).unwrap();
        assert_eq!(sarc.endian(), Endian::Big);
        assert_eq!(sarc.len(), 10);
        assert_eq!(sarc.guess_min_alignment(), 4);
        for file in &[
            "NavMesh/CDungeon/Dungeon119/Dungeon119.shknm2",
            "Map/CDungeon/Dungeon119/Dungeon119_Static.smubin",
            "Map/CDungeon/Dungeon119/Dungeon119_Dynamic.smubin",
            "Actor/Pack/DgnMrgPrt_Dungeon119.sbactorpack",
            "Physics/StaticCompound/CDungeon/Dungeon119.shksc",
            "Map/CDungeon/Dungeon119/Dungeon119_TeraTree.sblwp",
            "Map/CDungeon/Dungeon119/Dungeon119_Clustering.sblwp",
            "Map/DungeonData/CDungeon/Dungeon119.bdgnenv",
            "Model/DgnMrgPrt_Dungeon119.sbfres",
            "Model/DgnMrgPrt_Dungeon119.Tex2.sbfres",
        ] {
            sarc.get(file)
                .unwrap_or_else(|| panic!("Could not find file {}", file));
        }
    }
}
