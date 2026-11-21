mod publication;
mod reclaim;
mod segment;
mod state;

#[cfg(test)]
mod tests;

pub use reclaim::PackedReclamation;

use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Error, Result, SearchHit,
    object_format::{hex, validate_identity},
    search::{rank_rows, validate_query},
};

use self::{
    publication::{Artifact, Digest, install, read_document, stage, stage_document},
    segment::Segment,
    state::{Batch, Location, State},
};

const PROFILE_FILE: &str = "PACKED";
const PROFILE_MAGIC: &[u8; 8] = b"LDSPRF02";
const MANIFEST_MAGIC: &[u8; 8] = b"LDSMAN02";

/// One logical record and its model/source vector identity.
#[derive(Clone, Copy)]
pub struct PackedRecord<'a> {
    pub id: &'a str,
    pub source: &'a str,
    pub vector: &'a [f32],
}

/// A single-profile packed store in a dedicated, existing local directory.
pub struct PackedStore {
    root: PathBuf,
    profile: Profile,
    profile_digest: Digest,
}

/// A search view that owns the immutable mappings for one generation.
pub struct PackedGeneration {
    number: u64,
    dimensions: usize,
    segments: Vec<Segment>,
    records: Vec<(String, Location)>,
    // Field drop order releases the pin after all segment mappings are closed.
    _lease: Option<File>,
}

/// Holds the writer lock while staging one atomic generation.
pub struct PackedUpdate<'a> {
    store: &'a PackedStore,
    _writer: File,
    previous: Option<Digest>,
    number: u64,
    segments: Vec<SegmentRef>,
    state: State,
    staged: bool,
    failed: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    model: String,
    dimensions: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    number: u64,
    previous: Option<Digest>,
    profile: Digest,
    segments: Vec<SegmentRef>,
}

struct Loaded {
    manifest: Manifest,
    digest: Option<Digest>,
    state: State,
    file: Option<File>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SegmentRef {
    digest: Digest,
    token: String,
}

impl SegmentRef {
    fn name(&self) -> Result<String> {
        if !valid_token(&self.token) {
            return Err(Error::InvalidFormat(
                "invalid segment name in manifest".into(),
            ));
        }
        Ok(format!("segment-{}-{}.lds", hex(&self.digest), self.token))
    }

    fn path(&self, root: &Path) -> Result<PathBuf> {
        Ok(root.join(self.name()?))
    }
}

fn valid_token(token: &str) -> bool {
    token.len() == 16 && token.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

impl PackedStore {
    /// Initialize an empty store without replacing an existing profile.
    pub fn create(root: impl AsRef<Path>, model: &str, dimensions: usize) -> Result<Self> {
        validate_identity(model, "profile")?;
        let dimensions = u32::try_from(dimensions)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| Error::InvalidInput("invalid packed profile dimensions".into()))?;
        let root = fs::canonicalize(root)?;
        if latest_number(&root)?.is_some() {
            return Err(Error::InvalidInput(
                "directory already has a packed generation".into(),
            ));
        }
        let profile = Profile {
            model: model.into(),
            dimensions,
        };
        let prepared = stage_document(&root, Artifact::Profile, PROFILE_MAGIC, &profile)?;
        install(prepared, &root.join(PROFILE_FILE), Artifact::Profile)?;
        Self::open(root)
    }

    /// Open the versioned profile. Published data is validated when opening a generation.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        let mut file = File::open(root.join(PROFILE_FILE))?;
        file.lock_shared()?;
        let (profile, profile_digest) = read_document::<Profile>(&mut file, PROFILE_MAGIC)?;
        if profile.model.trim().is_empty() || profile.dimensions == 0 {
            return Err(Error::InvalidFormat("invalid packed profile".into()));
        }
        Ok(Self {
            root,
            profile,
            profile_digest,
        })
    }

    /// Select and validate the latest complete, published generation.
    pub fn open_current(&self) -> Result<PackedGeneration> {
        let _reader = self.profile_lock(false)?;
        let Loaded {
            manifest,
            state,
            file,
            ..
        } = self.load()?;
        if let Some(lease) = &file {
            lease.lock_shared()?;
        }
        let (segments, records) = state.into_view();
        Ok(PackedGeneration {
            number: manifest.number,
            dimensions: self.profile.dimensions as usize,
            segments,
            records,
            _lease: file,
        })
    }

    /// Atomically append a single batch.
    pub fn append(&self, rows: &[PackedRecord<'_>]) -> Result<u64> {
        let mut update = self.begin_append()?;
        update.stage(rows)?;
        update.commit()
    }

    /// Stage multiple bounded batches under one writer lock and publish them together.
    pub fn begin_append(&self) -> Result<PackedUpdate<'_>> {
        let writer = self.profile_lock(true)?;
        let Loaded {
            manifest,
            digest: previous,
            state,
            ..
        } = self.load()?;
        let number = next_number(manifest.number)?;
        Ok(PackedUpdate {
            store: self,
            _writer: writer,
            previous,
            number,
            segments: manifest.segments,
            state,
            staged: false,
            failed: false,
        })
    }

    /// Repack the current generation into fewer immutable segments.
    ///
    /// Each replacement segment contains at most the given number of distinct
    /// source rows. Returns the published generation, or None if the segment
    /// count cannot decrease. Reclamation retires older files independently.
    pub fn compact(&self, max_rows_per_segment: usize) -> Result<Option<u64>> {
        if max_rows_per_segment == 0 {
            return Err(Error::InvalidInput(
                "packed compaction requires a positive segment row limit".into(),
            ));
        }
        let _writer = self.profile_lock(true)?;
        let Loaded {
            manifest: current,
            digest: previous,
            state,
            ..
        } = self.load()?;
        let expected = state.vector_count().div_ceil(max_rows_per_segment);
        if expected >= current.segments.len() {
            return Ok(None);
        }

        let number = next_number(current.number)?;
        let mut replacement = State::default();
        let mut segments = Vec::with_capacity(expected);
        for batch in state.compaction_batches(max_rows_per_segment)? {
            let (reference, decoded) = self.write_segment(&batch)?;
            replacement.add(decoded, &self.profile.model)?;
            segments.push(reference);
        }
        state.validate_compaction(&replacement)?;
        self.publish(number, previous, segments).map(Some)
    }

    fn write_segment(&self, batch: &Batch<'_>) -> Result<(SegmentRef, segment::Decoded)> {
        let dimensions = self.profile.dimensions as usize;
        let staged = stage(&self.root, Artifact::Segment, |file| {
            segment::write(file, dimensions, &batch.vectors, &batch.bindings)
        })?;
        let digest = *blake3::Hasher::new()
            .update_reader(staged.reopen()?)?
            .finalize()
            .as_bytes();
        let token = staged
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(".stage-"))
            .expect("generated staging name")
            .into();
        let reference = SegmentRef { digest, token };
        let path = reference.path(&self.root)?;
        // Each attempt gets a fresh final name and its own durability barrier.
        install(staged, &path, Artifact::Segment)?;
        Ok((reference, Segment::open(&path, &digest, dimensions)?))
    }

    fn publish(
        &self,
        number: u64,
        previous: Option<Digest>,
        segments: Vec<SegmentRef>,
    ) -> Result<u64> {
        let manifest = Manifest {
            number,
            previous,
            profile: self.profile_digest,
            segments,
        };
        let staged = stage_document(&self.root, Artifact::Manifest, MANIFEST_MAGIC, &manifest)?;
        install(
            staged,
            &manifest_path(&self.root, number),
            Artifact::Manifest,
        )?;
        Ok(number)
    }

    fn profile_lock(&self, write: bool) -> Result<File> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(write)
            .open(self.root.join(PROFILE_FILE))?;
        if write {
            file.lock()?;
        } else {
            file.lock_shared()?;
        }
        let (_, digest) = read_document::<Profile>(&mut file, PROFILE_MAGIC)?;
        if digest != self.profile_digest {
            return Err(Error::InvalidFormat(
                "packed profile differs from the opened store".into(),
            ));
        }
        Ok(file)
    }

    fn load(&self) -> Result<Loaded> {
        let mut state = State::default();
        let Some(number) = latest_number(&self.root)? else {
            return Ok(Loaded {
                manifest: Manifest {
                    number: 0,
                    previous: None,
                    profile: self.profile_digest,
                    segments: Vec::new(),
                },
                digest: None,
                state,
                file: None,
            });
        };
        let mut file = File::open(manifest_path(&self.root, number))?;
        let (manifest, digest) = self.read_manifest(&mut file, number)?;
        for reference in &manifest.segments {
            let segment = Segment::open(
                &reference.path(&self.root)?,
                &reference.digest,
                self.profile.dimensions as usize,
            )?;
            state.add(segment, &self.profile.model)?;
        }
        Ok(Loaded {
            manifest,
            digest: Some(digest),
            state,
            file: Some(file),
        })
    }

    fn read_manifest(&self, file: &mut File, number: u64) -> Result<(Manifest, Digest)> {
        let (manifest, digest) = read_document::<Manifest>(file, MANIFEST_MAGIC)?;
        if manifest.number != number
            || manifest.profile != self.profile_digest
            || manifest.previous.is_some() != (number > 1)
            || manifest.segments.is_empty()
        {
            return Err(Error::InvalidFormat(
                "invalid packed generation manifest".into(),
            ));
        }
        Ok((manifest, digest))
    }
}

impl PackedUpdate<'_> {
    /// Validate and flush one batch. Dropping the update leaves the current generation alone.
    /// After a stage fails, drop the update and start a new one.
    pub fn stage(&mut self, rows: &[PackedRecord<'_>]) -> Result<()> {
        if self.failed {
            return Err(Error::InvalidInput("packed update has failed".into()));
        }
        let result = self.stage_batch(rows);
        self.failed = result.is_err();
        self.staged |= result.is_ok();
        result
    }

    /// Publish a complete manifest, then acknowledge the generation number.
    pub fn commit(self) -> Result<u64> {
        if self.failed || !self.staged {
            return Err(Error::InvalidInput(
                "packed update is empty or failed".into(),
            ));
        }
        self.store
            .publish(self.number, self.previous, self.segments)
    }

    fn stage_batch(&mut self, rows: &[PackedRecord<'_>]) -> Result<()> {
        let model = &self.store.profile.model;
        let dimensions = self.store.profile.dimensions as usize;
        let batch = self.state.prepare(rows, model, dimensions)?;
        if batch.bindings.is_empty() {
            // Publish a fresh manifest on an exact retry. This completes the durability
            // barrier even if the preceding attempt died after naming its manifest.
            return Ok(());
        }

        let (reference, decoded) = self.store.write_segment(&batch)?;
        self.state.add(decoded, model)?;
        self.segments.push(reference);
        Ok(())
    }
}

impl PackedGeneration {
    /// The manifest number, or zero for an initialized empty store.
    pub fn number(&self) -> u64 {
        self.number
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Number of live segment files in this view.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
    /// Exact dot-product top-K with ascending record ID as the tie-break.
    pub fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<SearchHit>> {
        validate_query(self.dimensions, query)?;
        let row = |index: usize| {
            let location = self.records[index].1;
            (index, self.segments[location.segment].row(location.row))
        };
        let range = 0..self.records.len();
        Ok(rank_rows(
            self.records.len(),
            query,
            top_k,
            range.clone().map(row),
            range.into_par_iter().map(row),
            |index| self.records[index].0.as_str(),
        ))
    }
}

fn next_number(number: u64) -> Result<u64> {
    number
        .checked_add(1)
        .ok_or_else(|| Error::InvalidInput("packed generation number exhausted".into()))
}

fn latest_number(root: &Path) -> Result<Option<u64>> {
    let mut latest = None;
    for entry in fs::read_dir(root)? {
        let name = entry?.file_name();
        if let Some(name) = name.to_str() {
            latest = latest.max(manifest_number(name, "generation-")?);
        }
    }
    Ok(latest)
}

fn manifest_number(name: &str, prefix: &str) -> Result<Option<u64>> {
    let Some(digits) = name
        .strip_prefix(prefix)
        .and_then(|name| name.strip_suffix(".ldg"))
    else {
        return Ok(None);
    };
    if digits.len() != 20 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(None);
    }
    let number = digits
        .parse::<u64>()
        .ok()
        .filter(|number| *number != 0)
        .ok_or_else(|| Error::InvalidFormat("invalid packed generation number".into()))?;
    Ok(Some(number))
}

fn manifest_path(root: &Path, number: u64) -> PathBuf {
    root.join(format!("generation-{number:020}.ldg"))
}
