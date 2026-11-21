use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use serde::{Serialize, de::DeserializeOwned};
use tempfile::{Builder, NamedTempFile};

use crate::{Error, Result};

pub(super) type Digest = [u8; 32];

const DOCUMENT_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(super) enum Artifact {
    Profile,
    Segment,
    Manifest,
    Retirement,
    Reclamation,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Boundary {
    Started,
    Written,
    Synced,
    Named,
    Durable,
    SegmentRemoved,
    MetadataRemoved,
}

pub(super) fn stage(
    root: &Path,
    artifact: Artifact,
    write: impl FnOnce(&mut File) -> Result<()>,
) -> Result<NamedTempFile> {
    let mut staged = Builder::new()
        .prefix(".stage-")
        .rand_bytes(16)
        .tempfile_in(root)?;
    checkpoint(artifact, Boundary::Started);
    write(staged.as_file_mut())?;
    checkpoint(artifact, Boundary::Written);
    staged.as_file().sync_all()?;
    checkpoint(artifact, Boundary::Synced);
    Ok(staged)
}

/// Publish to a fresh name in the same directory. Existing names are immutable.
pub(super) fn install(staged: NamedTempFile, target: &Path, artifact: Artifact) -> Result<()> {
    let (file, source) = staged.into_parts();
    drop(file);
    name_file(&source, target)?;
    checkpoint(artifact, Boundary::Named);
    sync_directory(target.parent().expect("store directory"))?;
    checkpoint(artifact, Boundary::Durable);
    Ok(())
}

pub(super) fn retire_manifest(source: &Path, target: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        // The profile writer lock excludes other retirement attempts.
        match target.symlink_metadata() {
            Ok(_) => return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        std::fs::rename(source, target)?;
    }
    #[cfg(windows)]
    name_file(source, target)?;
    checkpoint(Artifact::Retirement, Boundary::Named);
    Ok(())
}

pub(super) fn sync_directory(directory: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(directory)?.sync_all()?;
    #[cfg(windows)]
    let _ = directory;
    Ok(())
}

#[cfg(unix)]
fn name_file(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::hard_link(source, target)
}

#[cfg(windows)]
fn name_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let source = wide(source);
    let target = wide(target);
    // SAFETY: these are live, NUL-terminated paths in the same canonical local directory.
    // Omitting REPLACE_EXISTING and COPY_ALLOWED admits only a fresh same-volume name.
    let ok = unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn stage_document(
    root: &Path,
    artifact: Artifact,
    magic: &[u8; 8],
    document: &impl Serialize,
) -> Result<NamedTempFile> {
    let encoded = serde_json::to_vec(document)?;
    if encoded.len() as u64 > DOCUMENT_LIMIT - 40 {
        return Err(Error::InvalidInput(
            "packed store document is too large".into(),
        ));
    }
    let digest = blake3::Hasher::new()
        .update(magic)
        .update(&encoded)
        .finalize();
    stage(root, artifact, |file| {
        file.write_all(magic)?;
        file.write_all(&encoded)?;
        file.write_all(digest.as_bytes())?;
        Ok(())
    })
}

pub(super) fn read_document<T: DeserializeOwned>(
    file: &mut File,
    magic: &[u8; 8],
) -> Result<(T, Digest)> {
    let length = file.metadata()?.len();
    if !(40..=DOCUMENT_LIMIT).contains(&length) {
        return Err(Error::InvalidFormat(
            "invalid packed store document size".into(),
        ));
    }
    let mut bytes = vec![0; length as usize];
    file.read_exact(&mut bytes)?;
    let body_end = bytes.len() - 32;
    let digest = blake3::hash(&bytes[..body_end]);
    if &bytes[..8] != magic || digest.as_bytes().as_slice() != &bytes[body_end..] {
        return Err(Error::InvalidFormat(
            "invalid packed store document identity".into(),
        ));
    }
    let decoded = serde_json::from_slice(&bytes[8..body_end])
        .map_err(|error| Error::InvalidFormat(format!("invalid packed store document: {error}")))?;
    Ok((decoded, *digest.as_bytes()))
}

pub(super) fn checkpoint(artifact: Artifact, boundary: Boundary) {
    #[cfg(test)]
    super::tests::checkpoint(artifact, boundary);
    #[cfg(not(test))]
    let _ = (artifact, boundary);
}
