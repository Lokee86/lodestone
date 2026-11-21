use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use memmap2::{Mmap, MmapOptions};

use crate::{Error, Result};

use super::publication::Digest;

const MAGIC: &[u8; 8] = b"LDSSEG02";
const HEADER: usize = 64;
const ALIGNMENT: usize = 64;

pub(super) struct Segment {
    mmap: Mmap,
    vectors_offset: usize,
    dimensions: usize,
}

pub(super) struct Decoded {
    pub(super) segment: Segment,
    pub(super) sources: Vec<String>,
    pub(super) bindings: Vec<(String, Digest)>,
}

impl Segment {
    pub(super) fn open(path: &Path, expected: &Digest, dimensions: usize) -> Result<Decoded> {
        let file = File::open(path)?;
        // SAFETY: the store only opens synced, immutable segment files. It never replaces,
        // edits, or truncates them. The mapping owns its lifetime independently of the lock.
        let mmap = unsafe { MmapOptions::new().map(&file)? };
        if blake3::hash(&mmap).as_bytes() != expected {
            return Err(invalid("packed segment checksum mismatch"));
        }
        if mmap.len() < HEADER
            || &mmap[..8] != MAGIC
            || mmap[12..16]
                .iter()
                .chain(&mmap[40..HEADER])
                .any(|byte| *byte != 0)
            || u32::from_le_bytes(mmap[8..12].try_into().unwrap()) as usize != dimensions
        {
            return Err(invalid("invalid packed segment header"));
        }
        let sources_count = count(&mmap, 16)?;
        let bindings_count = count(&mmap, 24)?;
        let metadata_len = count(&mmap, 32)?;
        let metadata_end = HEADER.checked_add(metadata_len).ok_or_else(layout_error)?;
        let vectors_offset = aligned(metadata_end).ok_or_else(layout_error)?;
        let vectors_bytes = sources_count
            .checked_mul(dimensions)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(layout_error)?;
        if vectors_offset.checked_add(vectors_bytes) != Some(mmap.len())
            || bindings_count == 0
            || sources_count > metadata_len / 5
            || bindings_count > metadata_len / 37
            || mmap[metadata_end..vectors_offset]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(layout_error());
        }

        let values = bytemuck::try_cast_slice::<u8, f32>(&mmap[vectors_offset..])
            .map_err(|_| invalid("unaligned packed segment vectors"))?;
        if values.iter().any(|value| !value.is_finite()) {
            return Err(invalid("non-finite packed segment value"));
        }

        let mut metadata = &mmap[HEADER..metadata_end];
        let mut sources = Vec::with_capacity(sources_count);
        for _ in 0..sources_count {
            sources.push(take_text(&mut metadata)?);
        }
        let mut bindings = Vec::with_capacity(bindings_count);
        for _ in 0..bindings_count {
            let id = take_text(&mut metadata)?;
            let object = metadata.split_off(..32).ok_or_else(layout_error)?;
            bindings.push((id, object.try_into().unwrap()));
        }
        if !metadata.is_empty() {
            return Err(layout_error());
        }
        Ok(Decoded {
            segment: Self {
                mmap,
                vectors_offset,
                dimensions,
            },
            sources,
            bindings,
        })
    }

    pub(super) fn row(&self, index: usize) -> &[f32] {
        let width = self.dimensions * 4;
        let start = self.vectors_offset + index * width;
        bytemuck::cast_slice(&self.mmap[start..start + width])
    }
}

pub(super) fn write(
    file: &mut File,
    dimensions: usize,
    vectors: &[(&str, &[f32])],
    bindings: &[(&str, Digest)],
) -> Result<()> {
    let mut metadata = Vec::new();
    for (source, _) in vectors {
        write_text(&mut metadata, source)?;
    }
    for (id, object) in bindings {
        write_text(&mut metadata, id)?;
        metadata.extend_from_slice(object);
    }
    let mut header = [0_u8; HEADER];
    header[..8].copy_from_slice(MAGIC);
    header[8..12].copy_from_slice(&(dimensions as u32).to_le_bytes());
    header[16..24].copy_from_slice(&(vectors.len() as u64).to_le_bytes());
    header[24..32].copy_from_slice(&(bindings.len() as u64).to_le_bytes());
    header[32..40].copy_from_slice(&(metadata.len() as u64).to_le_bytes());

    let mut writer = BufWriter::with_capacity(1024 * 1024, file);
    writer.write_all(&header)?;
    writer.write_all(&metadata)?;
    let padding = (ALIGNMENT - metadata.len() % ALIGNMENT) % ALIGNMENT;
    writer.write_all(&[0; ALIGNMENT][..padding])?;
    for (_, vector) in vectors {
        writer.write_all(bytemuck::cast_slice(vector))?;
    }
    writer.flush()?;
    Ok(())
}

fn count(bytes: &[u8], offset: usize) -> Result<usize> {
    let value = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    usize::try_from(value).map_err(|_| layout_error())
}

fn aligned(offset: usize) -> Option<usize> {
    offset
        .checked_add(ALIGNMENT - 1)
        .map(|value| value & !(ALIGNMENT - 1))
}

fn take_text(bytes: &mut &[u8]) -> Result<String> {
    let length = bytes.split_off(..4).ok_or_else(layout_error)?;
    let length = u32::from_le_bytes(length.try_into().unwrap()) as usize;
    let text = bytes.split_off(..length).ok_or_else(layout_error)?;
    let text = std::str::from_utf8(text).map_err(|_| invalid("packed identity must be UTF-8"))?;
    if text.trim().is_empty() {
        return Err(invalid("packed identity is empty"));
    }
    Ok(text.to_owned())
}

fn write_text(bytes: &mut Vec<u8>, text: &str) -> Result<()> {
    let length = u32::try_from(text.len())
        .map_err(|_| Error::InvalidInput("packed identity is too long".into()))?;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

fn layout_error() -> Error {
    invalid("invalid packed segment layout")
}

fn invalid(message: &str) -> Error {
    Error::InvalidFormat(message.into())
}
