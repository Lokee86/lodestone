use std::{cmp::Ordering, collections::BinaryHeap, convert::Infallible};

use rayon::prelude::*;

use crate::{Error, PackedVectors, Result, ScalarType, Snapshot};

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub score: f32,
    pub index: usize,
}

/// A result identified by its row ordinal in an application-owned matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedCosineHit {
    pub index: usize,
    pub score: f64,
}

trait Score: Copy {
    fn compare(self, other: Self) -> Ordering;
}

impl Score for f32 {
    fn compare(self, other: Self) -> Ordering {
        self.total_cmp(&other)
    }
}

impl Score for f64 {
    fn compare(self, other: Self) -> Ordering {
        self.total_cmp(&other)
    }
}

#[derive(Clone, Copy, Debug)]
struct Candidate<S> {
    index: usize,
    score: S,
}

impl<S: Score> Eq for Candidate<S> {}

impl<S: Score> PartialEq for Candidate<S> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl<S: Score> Ord for Candidate<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .score
            .compare(self.score)
            .then_with(|| self.index.cmp(&other.index))
    }
}

impl<S: Score> PartialOrd for Candidate<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn search(snapshot: &Snapshot, query: &[f32], top_k: usize) -> Result<Vec<SearchHit>> {
    let dimensions = snapshot.info().dimensions;
    validate_query(dimensions, query)?;
    let vectors = snapshot.vectors();
    Ok(rank_rows(
        snapshot.info().count,
        query,
        top_k,
        vectors.chunks_exact(dimensions).enumerate(),
        vectors.par_chunks_exact(dimensions).enumerate(),
        |index| snapshot.id(index),
    ))
}

pub(crate) fn validate_query(dimensions: usize, query: &[f32]) -> Result<()> {
    if query.len() != dimensions || query.iter().any(|value| !value.is_finite()) {
        return Err(Error::InvalidInput(
            "query dimensions or values are invalid".into(),
        ));
    }
    Ok(())
}

/// Exhaustively rank application-owned, little-endian F32 rows by cosine similarity.
///
/// The matrix is borrowed in place. Zero rows have no cosine score and are omitted.
/// Tied scores use ascending row ordinal. The caller owns publication and identity.
pub fn search_packed_cosine(
    matrix: &PackedVectors,
    query: &[f32],
    top_k: usize,
) -> Result<Vec<PackedCosineHit>> {
    let schema = matrix.schema();
    if schema.scalar != ScalarType::F32 {
        return Err(Error::InvalidInput(
            "cosine requires a packed F32 matrix".into(),
        ));
    }
    validate_query(schema.dimensions as usize, query)?;
    let query_norm = query
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>()
        .sqrt();
    if query_norm == 0.0 {
        return Err(Error::InvalidInput("cosine query has zero norm".into()));
    }
    let width = size_of_val(query);
    let score = |(index, bytes): (usize, &[u8])| {
        let mut dot = 0.0;
        let mut norm_squared = 0.0;
        for (bytes, &query_value) in bytes.as_chunks::<4>().0.iter().zip(query) {
            let value = f64::from(f32::from_le_bytes(*bytes));
            dot += f64::from(query_value) * value;
            norm_squared += value * value;
        }
        if !norm_squared.is_finite() {
            return Err(Error::InvalidFormat("non-finite packed cosine row".into()));
        }
        Ok((norm_squared > 0.0).then(|| Candidate {
            index,
            score: (dot / (query_norm * norm_squared.sqrt())).clamp(-1.0, 1.0),
        }))
    };
    let bytes = matrix.bytes();
    Ok(select_top_k(
        matrix.count(),
        top_k,
        bytes.chunks_exact(width).enumerate().map(score),
        bytes.par_chunks_exact(width).enumerate().map(score),
    )?
    .into_iter()
    .map(|candidate| PackedCosineHit {
        index: candidate.index,
        score: candidate.score,
    })
    .collect())
}

pub(crate) fn rank_rows<'a>(
    count: usize,
    query: &[f32],
    top_k: usize,
    serial: impl Iterator<Item = (usize, &'a [f32])>,
    parallel: impl ParallelIterator<Item = (usize, &'a [f32])>,
    id: impl Fn(usize) -> &'a str,
) -> Vec<SearchHit> {
    let score = |(index, vector): (usize, &[f32])| {
        Ok::<_, Infallible>(Some(Candidate {
            index,
            score: dot(query, vector),
        }))
    };
    select_top_k(count, top_k, serial.map(score), parallel.map(score))
        .unwrap_or_else(|error| match error {})
        .into_iter()
        .map(|candidate| SearchHit {
            id: id(candidate.index).to_owned(),
            score: candidate.score,
            index: candidate.index,
        })
        .collect()
}

fn select_top_k<S: Score + Send, E: Send>(
    count: usize,
    top_k: usize,
    mut serial: impl Iterator<Item = std::result::Result<Option<Candidate<S>>, E>>,
    parallel: impl ParallelIterator<Item = std::result::Result<Option<Candidate<S>>, E>>,
) -> std::result::Result<Vec<Candidate<S>>, E> {
    let top_k = top_k.min(count);
    if top_k == 0 {
        return Ok(Vec::new());
    }
    let new_heap = || BinaryHeap::with_capacity(top_k);
    let insert = |mut heap, candidate: std::result::Result<Option<Candidate<S>>, E>| {
        if let Some(candidate) = candidate? {
            push(&mut heap, candidate, top_k);
        }
        Ok(heap)
    };
    let heap = if count < 2048 {
        serial.try_fold(new_heap(), insert)?
    } else {
        parallel
            .try_fold(new_heap, insert)
            .try_reduce(new_heap, |mut left, right| {
                for candidate in right {
                    push(&mut left, candidate, top_k);
                }
                Ok(left)
            })?
    };
    Ok(heap.into_sorted_vec())
}

fn push<S: Score>(heap: &mut BinaryHeap<Candidate<S>>, candidate: Candidate<S>, top_k: usize) {
    if heap.len() < top_k {
        heap.push(candidate);
        return;
    }
    let mut worst = heap.peek_mut().expect("non-empty top-k heap");
    if candidate < *worst {
        *worst = candidate;
    }
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObjectStore, RecordRef, materialize};

    #[test]
    fn borrowed_packed_rows_preserve_exact_cosine_and_ordinal_ties() {
        let schema = crate::VectorSchema::new(2, ScalarType::F32).unwrap();
        let values = [1.0_f32, 0.0, -1.0, 0.0, 2.0, 0.0, 0.0, 0.0];
        let matrix = PackedVectors::from_bytes(
            schema,
            values.into_iter().flat_map(f32::to_le_bytes).collect(),
        )
        .unwrap();
        assert_eq!(
            search_packed_cosine(&matrix, &[1.0, 0.0], 5).unwrap(),
            [
                PackedCosineHit {
                    index: 0,
                    score: 1.0
                },
                PackedCosineHit {
                    index: 2,
                    score: 1.0
                },
                PackedCosineHit {
                    index: 1,
                    score: -1.0
                },
            ]
        );
        assert!(search_packed_cosine(&matrix, &[0.0, 0.0], 1).is_err());
        let invalid = PackedVectors::from_bytes(
            schema,
            [f32::NAN, 0.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap();
        assert!(search_packed_cosine(&invalid, &[1.0, 0.0], 1).is_err());
    }

    #[test]
    fn returns_exact_top_results_with_stable_ties() {
        let temp = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(temp.path().join("store"));
        store.put("m", "a", &[1.0, 0.0]).unwrap();
        store.put("m", "b", &[0.5, 0.5]).unwrap();
        store.put("m", "c", &[0.0, 1.0]).unwrap();
        let records = [
            RecordRef {
                id: "a".into(),
                source: "a".into(),
            },
            RecordRef {
                id: "b".into(),
                source: "b".into(),
            },
            RecordRef {
                id: "c".into(),
                source: "c".into(),
            },
        ];
        let path = temp.path().join("snapshot.gvs");
        materialize(&store, "m", &records, &path).unwrap();
        let snapshot = Snapshot::open(path).unwrap();
        let hits = search(&snapshot, &[1.0, 0.0], 2).unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }
}
