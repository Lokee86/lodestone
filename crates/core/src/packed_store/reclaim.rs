use std::{
    collections::HashSet,
    fs::{self, File, TryLockError},
    path::Path,
};

use crate::{Error, Result};

use super::{
    Loaded, PackedStore, manifest_number,
    publication::{Artifact, Boundary, checkpoint, retire_manifest, sync_directory},
    valid_token,
};

/// Files reclaimed by one successful maintenance pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PackedReclamation {
    pub retired_generations: usize,
    pub removed_segments: usize,
}

impl PackedStore {
    /// Reclaim segment files outside the current generation and all open views.
    ///
    /// New views and writers wait during cleanup. Pinned views can still search.
    /// An interrupted pass may be restarted against the same directory.
    pub fn reclaim(&self) -> Result<PackedReclamation> {
        let _writer = self.profile_lock(true)?;
        // Verify the current generation, including all of its segments, before removing
        // any previously published data. An initialized empty store has no retirement root.
        let Loaded {
            manifest: current,
            state,
            file,
            ..
        } = self.load()?;
        drop(state);
        drop(file);
        if current.number == 0 {
            return Ok(PackedReclamation::default());
        }
        let mut protected = HashSet::new();
        for reference in &current.segments {
            protect(&self.root, &reference.name()?, &mut protected)?;
        }
        let mut retirements = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(number) = manifest_number(name, "generation-")? else {
                continue;
            };
            if !entry.file_type()?.is_file() {
                return Err(Error::InvalidFormat(
                    "packed manifest must be a regular file".into(),
                ));
            }
            if number == current.number {
                continue;
            }
            let mut file = File::open(entry.path())?;
            let pinned = match file.try_lock() {
                Ok(()) => false,
                Err(TryLockError::WouldBlock) => true,
                Err(TryLockError::Error(error)) => return Err(error.into()),
            };
            let (manifest, _) = self.read_manifest(&mut file, number)?;
            for reference in &manifest.segments {
                let name = reference.name()?;
                if pinned {
                    protect(&self.root, &name, &mut protected)?;
                }
            }
            if !pinned {
                // The profile lock closes admission throughout retirement. Release
                // the probe handle so the name can be moved on Windows.
                drop(file);
                retirements.push((number, entry.path()));
            }
        }
        retirements.sort_unstable_by_key(|(number, _)| *number);
        for (number, source) in &retirements {
            let target = self.root.join(format!(".retired-{number:020}.ldg"));
            retire_manifest(source, &target)?;
        }

        // Also cover names left by a previous process interrupted before its sync.
        sync_directory(&self.root)?;
        checkpoint(Artifact::Retirement, Boundary::Durable);

        let removed_segments = sweep(&self.root, &protected)?;
        Ok(PackedReclamation {
            retired_generations: retirements.len(),
            removed_segments,
        })
    }
}

fn protect(root: &Path, name: &str, protected: &mut HashSet<String>) -> Result<()> {
    if !fs::symlink_metadata(root.join(name))?.file_type().is_file() {
        return Err(Error::InvalidFormat(
            "retained packed segment must be a regular file".into(),
        ));
    }
    // The on-disk spelling can differ on a case-insensitive volume.
    protected.insert(name.to_ascii_lowercase());
    Ok(())
}

fn sweep(root: &Path, protected: &HashSet<String>) -> Result<usize> {
    let mut segments = Vec::new();
    let mut debris = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if is_segment(name) && !protected.contains(&name.to_ascii_lowercase()) {
            segments.push(entry.path());
        } else if name.strip_prefix(".stage-").is_some_and(valid_token)
            || manifest_number(name, ".retired-").is_ok_and(|number| number.is_some())
        {
            debris.push(entry.path());
        }
    }
    for path in &segments {
        fs::remove_file(path)?;
        checkpoint(Artifact::Reclamation, Boundary::SegmentRemoved);
    }
    for path in &debris {
        fs::remove_file(path)?;
        checkpoint(Artifact::Reclamation, Boundary::MetadataRemoved);
    }
    if !segments.is_empty() || !debris.is_empty() {
        sync_directory(root)?;
    }
    Ok(segments.len())
}

pub(super) fn is_segment(name: &str) -> bool {
    name.strip_prefix("segment-")
        .and_then(|name| name.strip_suffix(".lds"))
        .and_then(|name| name.split_once('-'))
        .is_some_and(|(digest, token)| {
            digest.len() == 64
                && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                && valid_token(token)
        })
}
