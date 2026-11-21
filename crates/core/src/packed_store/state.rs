use std::collections::HashMap;

use crate::{Error, Result, object_format::address};

use super::{
    PackedRecord,
    publication::Digest,
    segment::{Decoded, Segment},
};

#[derive(Clone, Copy)]
pub(super) struct Location {
    pub(super) segment: usize,
    pub(super) row: usize,
}

struct Vector {
    name: String,
    location: Location,
}

#[derive(Default)]
pub(super) struct State {
    segments: Vec<Segment>,
    objects: HashMap<Digest, Vector>,
    records: HashMap<String, Digest>,
}

pub(super) struct Batch<'a> {
    pub(super) vectors: Vec<(&'a str, &'a [f32])>,
    pub(super) bindings: Vec<(&'a str, Digest)>,
}

impl State {
    pub(super) fn vector_count(&self) -> usize {
        self.objects.len()
    }

    pub(super) fn compaction_batches(&self, max_rows: usize) -> Result<Vec<Batch<'_>>> {
        let mut objects: Vec<_> = self.objects.values().collect();
        objects.sort_unstable_by_key(|object| object.name.as_str());
        let mut bindings: Vec<_> = self
            .records
            .iter()
            .map(|(id, digest)| (self.objects[digest].name.as_str(), id.as_str(), *digest))
            .collect();
        bindings.sort_unstable_by_key(|(source, id, _)| (*source, *id));

        let mut remaining = bindings.as_slice();
        let mut batches = Vec::new();
        for group in objects.chunks(max_rows) {
            let last_source = group.last().unwrap().name.as_str();
            let split = remaining.partition_point(|(source, _, _)| *source <= last_source);
            let (linked, rest) = remaining.split_at(split);
            remaining = rest;
            if linked.is_empty() {
                return Err(Error::InvalidFormat(
                    "packed compaction found a group of sources without record bindings".into(),
                ));
            }
            let vectors = group
                .iter()
                .map(|object| (object.name.as_str(), self.row(object.location)))
                .collect();
            let mut bindings: Vec<_> = linked
                .iter()
                .map(|(_, id, digest)| (*id, *digest))
                .collect();
            bindings.sort_unstable_by_key(|(id, _)| *id);
            batches.push(Batch { vectors, bindings });
        }
        Ok(batches)
    }

    pub(super) fn validate_compaction(&self, replacement: &Self) -> Result<()> {
        if self.records != replacement.records
            || self.objects.len() != replacement.objects.len()
            || self.objects.iter().any(|(id, object)| {
                !replacement.objects.get(id).is_some_and(|new| {
                    object.name == new.name
                        && equal_vector_bytes(
                            self.row(object.location),
                            replacement.row(new.location),
                        )
                })
            })
        {
            return Err(Error::InvalidFormat(
                "packed compaction changed a record binding or vector".into(),
            ));
        }
        Ok(())
    }

    fn row(&self, location: Location) -> &[f32] {
        self.segments[location.segment].row(location.row)
    }

    pub(super) fn prepare<'a>(
        &self,
        rows: &[PackedRecord<'a>],
        model: &str,
        dimensions: usize,
    ) -> Result<Batch<'a>> {
        if rows.is_empty() {
            return Err(Error::InvalidInput("packed batch is empty".into()));
        }
        let mut vectors = HashMap::new();
        let mut bindings = HashMap::new();
        for row in rows {
            if row.id.trim().is_empty()
                || row.source.trim().is_empty()
                || row.vector.len() != dimensions
                || row.vector.iter().any(|value| !value.is_finite())
            {
                return Err(Error::InvalidInput(
                    "invalid packed record or vector".into(),
                ));
            }
            let key = address(model, row.source);
            if let Some(existing) = self.objects.get(&key) {
                check_vector(&existing.name, self.row(existing.location), row)?;
            } else {
                let (source, vector) = vectors.entry(key).or_insert((row.source, row.vector));
                check_vector(source, vector, row)?;
            }
            if let Some(existing) = self.records.get(row.id) {
                check_binding(*existing, key)?;
            } else {
                check_binding(*bindings.entry(row.id).or_insert(key), key)?;
            }
        }
        let mut vectors: Vec<_> = vectors.into_values().collect();
        vectors.sort_unstable_by_key(|(source, _)| *source);
        let mut bindings: Vec<_> = bindings.into_iter().collect();
        bindings.sort_unstable_by_key(|(id, _)| *id);
        Ok(Batch { vectors, bindings })
    }

    pub(super) fn add(&mut self, decoded: Decoded, model: &str) -> Result<()> {
        let segment = self.segments.len();
        for (row, name) in decoded.sources.into_iter().enumerate() {
            let digest = address(model, &name);
            if self
                .objects
                .insert(
                    digest,
                    Vector {
                        name,
                        location: Location { segment, row },
                    },
                )
                .is_some()
            {
                return Err(Error::InvalidFormat(
                    "packed generation repeats a source identity".into(),
                ));
            }
        }
        for (id, object) in decoded.bindings {
            if !self.objects.contains_key(&object) || self.records.insert(id, object).is_some() {
                return Err(Error::InvalidFormat(
                    "invalid or repeated packed record binding".into(),
                ));
            }
        }
        self.segments.push(decoded.segment);
        Ok(())
    }

    pub(super) fn into_view(self) -> (Vec<Segment>, Vec<(String, Location)>) {
        let mut records: Vec<_> = self
            .records
            .into_iter()
            .map(|(id, object)| (id, self.objects[&object].location))
            .collect();
        records.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        (self.segments, records)
    }
}

fn check_binding(existing: Digest, incoming: Digest) -> Result<()> {
    if existing != incoming {
        return Err(Error::InvalidInput(
            "packed record id refers to another source".into(),
        ));
    }
    Ok(())
}

fn check_vector(source: &str, vector: &[f32], incoming: &PackedRecord<'_>) -> Result<()> {
    if source != incoming.source || !equal_vector_bytes(vector, incoming.vector) {
        return Err(Error::InvalidInput(
            "model/source address already contains different vector bytes".into(),
        ));
    }
    Ok(())
}

fn equal_vector_bytes(left: &[f32], right: &[f32]) -> bool {
    bytemuck::cast_slice::<f32, u8>(left) == bytemuck::cast_slice::<f32, u8>(right)
}
