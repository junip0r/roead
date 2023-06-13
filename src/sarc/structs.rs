use core::mem::size_of;

use byte::{ctx, BytesExt, Error, TryRead, TryWrite};

use super::*;
use crate::Endian;

impl TryRead<'_> for ResHeader {
    fn try_read(bytes: &'_ [u8], _ctx: ()) -> byte::Result<(Self, usize)> {
        if bytes.len() < size_of::<Self>() {
            Err(Error::Incomplete)
        } else {
            let (bom, _) = Endian::try_read(&bytes[2..4], ())?;
            let endian = bom.into();
            let (header_size, _) = u16::try_read(bytes, endian)?;
            let mut offset = size_of::<u16>() + size_of::<Endian>();
            Ok((
                Self {
                    header_size,
                    bom,
                    file_size: bytes.read_with(&mut offset, endian)?,
                    data_offset: bytes.read_with(&mut offset, endian)?,
                    version: bytes.read_with(&mut offset, endian)?,
                    reserved: bytes.read_with(&mut offset, endian)?,
                },
                size_of::<Self>(),
            ))
        }
    }
}

impl TryWrite for ResHeader {
    fn try_write(self, bytes: &mut [u8], _ctx: ()) -> byte::Result<usize> {
        if bytes.len() < size_of::<Self>() {
            Err(Error::Incomplete)
        } else {
            let endian = self.bom.into();
            let offset = &mut 0;
            bytes.write_with(offset, self.header_size, endian)?;
            bytes.write_with(offset, self.bom, ())?;
            bytes.write_with(offset, self.file_size, endian)?;
            bytes.write_with(offset, self.data_offset, endian)?;
            bytes.write_with(offset, self.version, endian)?;
            bytes.write_with(offset, self.reserved, endian)?;
            Ok(size_of::<Self>())
        }
    }
}

impl ResFatHeader {
    pub(crate) const MAGIC: &[u8] = b"SFAT";
}

impl TryRead<'_, ctx::Endian> for ResFatHeader {
    fn try_read(bytes: &[u8], ctx: ctx::Endian) -> byte::Result<(Self, usize)> {
        if bytes.len() < Self::MAGIC.len() + size_of::<Self>() {
            Err(Error::Incomplete)
        } else if &bytes[..Self::MAGIC.len()] != Self::MAGIC {
            Err(Error::BadInput {
                err: "Bad SFAT header magic",
            })
        } else {
            let offset = &mut Self::MAGIC.len();
            Ok((
                Self {
                    header_size: bytes.read_with(offset, ctx)?,
                    num_files: bytes.read_with(offset, ctx)?,
                    hash_multiplier: bytes.read_with(offset, ctx)?,
                },
                *offset,
            ))
        }
    }
}

impl TryWrite<ctx::Endian> for ResFatHeader {
    fn try_write(self, bytes: &mut [u8], ctx: ctx::Endian) -> byte::Result<usize> {
        if bytes.len() < Self::MAGIC.len() + size_of::<Self>() {
            Err(Error::Incomplete)
        } else {
            let offset = &mut 0;
            bytes.write_with(offset, Self::MAGIC, ())?;
            bytes.write_with(offset, self.header_size, ctx)?;
            bytes.write_with(offset, self.num_files, ctx)?;
            bytes.write_with(offset, self.hash_multiplier, ctx)?;
            Ok(*offset)
        }
    }
}

impl TryRead<'_, ctx::Endian> for ResFatEntry {
    fn try_read(bytes: &'_ [u8], ctx: ctx::Endian) -> byte::Result<(Self, usize)> {
        if bytes.len() < size_of::<Self>() {
            Err(Error::Incomplete)
        } else {
            let offset = &mut 0;
            Ok((
                Self {
                    name_hash: bytes.read_with(offset, ctx)?,
                    rel_name_opt_offset: bytes.read_with(offset, ctx)?,
                    data_begin: bytes.read_with(offset, ctx)?,
                    data_end: bytes.read_with(offset, ctx)?,
                },
                *offset,
            ))
        }
    }
}

impl TryWrite<ctx::Endian> for ResFatEntry {
    fn try_write(self, bytes: &mut [u8], ctx: ctx::Endian) -> byte::Result<usize> {
        if bytes.len() < size_of::<Self>() {
            Err(Error::Incomplete)
        } else {
            let offset = &mut 0;
            bytes.write_with(offset, self.name_hash, ctx)?;
            bytes.write_with(offset, self.rel_name_opt_offset, ctx)?;
            bytes.write_with(offset, self.data_begin, ctx)?;
            bytes.write_with(offset, self.data_end, ctx)?;
            Ok(*offset)
        }
    }
}

impl ResFntHeader {
    pub(crate) const MAGIC: &[u8] = b"SFNT";
}

impl TryRead<'_, ctx::Endian> for ResFntHeader {
    fn try_read(bytes: &'_ [u8], ctx: ctx::Endian) -> byte::Result<(Self, usize)> {
        if bytes.len() < Self::MAGIC.len() + size_of::<Self>() {
            Err(Error::Incomplete)
        } else if &bytes[..Self::MAGIC.len()] != Self::MAGIC {
            Err(Error::BadInput {
                err: "Bad SFNT header magic",
            })
        } else {
            let offset = &mut Self::MAGIC.len();
            Ok((
                Self {
                    header_size: bytes.read_with(offset, ctx)?,
                    reserved: bytes.read_with(offset, ctx)?,
                },
                *offset,
            ))
        }
    }
}

impl TryWrite<ctx::Endian> for ResFntHeader {
    fn try_write(self, bytes: &mut [u8], ctx: ctx::Endian) -> byte::Result<usize> {
        if bytes.len() < Self::MAGIC.len() + size_of::<Self>() {
            Err(Error::Incomplete)
        } else {
            let offset = &mut 0;
            bytes.write_with(offset, Self::MAGIC, ())?;
            bytes.write_with(offset, self.header_size, ctx)?;
            bytes.write_with(offset, self.reserved, ctx)?;
            Ok(*offset)
        }
    }
}
