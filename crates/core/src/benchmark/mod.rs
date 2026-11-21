mod data;
mod footprint;
mod image;

use std::{
    env, fs,
    hint::black_box,
    ops::Range,
    path::{Path, PathBuf},
    time::Instant,
};

use rayon::prelude::*;
use serde_json::{Value, json};

use crate::{
    Error, ObjectStore, PackedGeneration, PackedRecord, PackedStore, RecordRef, Result, SearchHit,
    Snapshot, materialize, search,
};

use self::data::{MODEL, Rows, SEED};

const SEGMENT_ROWS: usize = 16_384;
const APPEND_ROWS: usize = 128;
const TOP_K: usize = 10;
const CONCURRENT_QUERIES: usize = 4;

#[test]
#[ignore = "writes local benchmark datasets and runs timed exact searches"]
fn benchmark_packed_segments() -> Result<()> {
    let fixture = env::var_os("LODESTONE_BENCH_INPUT").map(PathBuf::from);
    let input = match &fixture {
        Some(path) => Rows::fixture(path)?,
        None => Rows::seeded_from_env()?,
    };
    let temp = match env::var_os("LODESTONE_BENCH_SCRATCH") {
        Some(path) => tempfile::tempdir_in(path)?,
        None => tempfile::tempdir()?,
    };
    let records = input.records();
    let queries = input.queries();
    let append_rows = fixture
        .as_ref()
        .map_or(0, |_| APPEND_ROWS.min(input.count / 8));
    let initial_rows = input.count - append_rows;
    let v1_dir = temp.path().join("v1");
    let packed_dir = temp.path().join("packed");
    fs::create_dir(&v1_dir)?;
    fs::create_dir(&packed_dir)?;
    let snapshot_path = v1_dir.join("search.gvs");
    let store = ObjectStore::new(v1_dir.join("store"));

    let started = Instant::now();
    if fixture.is_some() {
        put_rows(&store, &input, &records, 0..initial_rows)?;
        materialize(&store, MODEL, &records[..initial_rows], &snapshot_path)?;
    } else {
        image::write_image(&snapshot_path, &input, &records, 0..initial_rows)?;
    }
    let v1_build_ms = elapsed_ms(started);

    let started = Instant::now();
    let packed = PackedStore::create(&packed_dir, MODEL, input.dimensions)?;
    assert_eq!(publish(&packed, &input, &records, 0..initial_rows)?, 1);
    let packed_build_ms = elapsed_ms(started);
    let append = if append_rows > 0 {
        let object_bytes_before = footprint::measure(&v1_dir.join("store"))?.apparent_bytes;
        let started = Instant::now();
        put_rows(&store, &input, &records, initial_rows..input.count)?;
        materialize(&store, MODEL, &records, &snapshot_path)?;
        let v1_ms = elapsed_ms(started);
        let v1_bytes = footprint::measure(&v1_dir.join("store"))?.apparent_bytes
            - object_bytes_before
            + fs::metadata(&snapshot_path)?.len();

        let packed_before = footprint::measure(&packed_dir)?.apparent_bytes;
        let started = Instant::now();
        assert_eq!(
            publish(&packed, &input, &records, initial_rows..input.count)?,
            2
        );
        let packed_ms = elapsed_ms(started);
        let packed_bytes = footprint::measure(&packed_dir)?.apparent_bytes - packed_before;
        Some(json!({
            "rows": append_rows,
            "v1_output_bytes": v1_bytes,
            "packed_output_bytes": packed_bytes,
            "v1_ms": v1_ms,
            "packed_ms": packed_ms,
        }))
    } else {
        None
    };

    let v1_disk = footprint::measure(&v1_dir)?;
    let packed_disk = footprint::measure(&packed_dir)?;
    let v1 = Snapshot::open(&snapshot_path)?;
    let candidate = PackedStore::open(&packed_dir)?.open_current()?;
    assert_eq!(candidate.record_count(), input.count);
    let (v1_search, packed_search) = compare_search(&v1, &candidate, &queries)?;
    let packed_generation = candidate.number();
    let segment_count = candidate.segment_count();
    drop(v1);
    let compaction = env::var_os("LODESTONE_BENCH_COMPACTION_ROWS")
        .map(|value| {
            let target = value
                .to_str()
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| {
                    Error::InvalidInput("invalid LODESTONE_BENCH_COMPACTION_ROWS".into())
                })?;
            verify_compaction(&packed, &packed_dir, candidate, &queries, target)
        })
        .transpose()?;
    let evidence = json!({
        "mode": if fixture.is_some() { "fixture-full-v1" } else { "seeded-search-only-v1" },
        "seed": fixture.is_none().then_some(SEED),
        "os": env::consts::OS,
        "architecture": env::consts::ARCH,
        "dimensions": input.dimensions,
        "rows": input.count,
        "raw_vector_bytes": input.dimensions * input.count * 4,
        "queries": queries.len(),
        "top_k": TOP_K,
        "exact_parity": true,
        "rayon_threads": rayon::current_num_threads(),
        "concurrent_queries": CONCURRENT_QUERIES,
        "target_segment_rows": SEGMENT_ROWS,
        "packed_generation": packed_generation,
        "segment_count": segment_count,
        "append": append,
        "compaction": compaction,
        "v1": { "initial_build_ms": v1_build_ms, "disk": v1_disk, "warm_search": v1_search },
        "packed": { "initial_build_ms": packed_build_ms, "disk": packed_disk, "warm_search": packed_search },
    });
    println!("LODESTONE_BENCHMARK {evidence}");
    Ok(())
}

fn verify_compaction(
    store: &PackedStore,
    directory: &Path,
    pinned: PackedGeneration,
    queries: &[Vec<f32>],
    target: usize,
) -> Result<Value> {
    let expected = queries
        .iter()
        .map(|query| pinned.search(query, TOP_K))
        .collect::<Result<Vec<_>>>()?;
    let started = Instant::now();
    let published = store.compact(target)?;
    let elapsed = elapsed_ms(started);
    let current = PackedStore::open(directory)?.open_current()?;
    assert_eq!(
        current.number(),
        published.unwrap_or_else(|| pinned.number())
    );
    assert_eq!(current.record_count(), pinned.record_count());
    let retained_disk = footprint::measure(directory)?;
    let while_pinned = store.reclaim()?;
    assert_eq!(
        while_pinned.removed_segments, 0,
        "original reader still pins the old segments"
    );
    for (index, (query, before)) in queries.iter().zip(&expected).enumerate() {
        assert_eq!(
            &current.search(query, TOP_K)?,
            before,
            "compaction parity on query {index}"
        );
        assert_eq!(
            &pinned.search(query, TOP_K)?,
            before,
            "pinned reader parity on query {index}"
        );
    }
    let segments_before = pinned.segment_count();
    drop(pinned);
    let reclaimed = store.reclaim()?;
    let expected_removed = if published.is_some() {
        segments_before
    } else {
        0
    };
    assert_eq!(reclaimed.removed_segments, expected_removed);
    let reopened = PackedStore::open(directory)?.open_current()?;
    assert_eq!(reopened.number(), current.number());
    for (index, (query, before)) in queries.iter().zip(expected).enumerate() {
        assert_eq!(
            reopened.search(query, TOP_K)?,
            before,
            "reclamation parity on query {index}"
        );
    }
    Ok(json!({
        "published_generation": published,
        "segments_before": segments_before,
        "segments_after": current.segment_count(),
        "target_rows": target,
        "queries": queries.len(),
        "exact_parity": true,
        "elapsed_ms": elapsed,
        "retained_disk": retained_disk,
        "reclamation": {
            "segments_removed_while_pinned": while_pinned.removed_segments,
            "segments_removed_after_close": reclaimed.removed_segments,
            "remaining_disk": footprint::measure(directory)?,
        },
    }))
}

fn put_rows(
    store: &ObjectStore,
    input: &Rows,
    records: &[RecordRef],
    range: Range<usize>,
) -> Result<()> {
    let mut row = vec![0.0; input.dimensions];
    for index in range {
        input.fill(index, &mut row);
        store.put(MODEL, &records[index].source, &row)?;
    }
    Ok(())
}

fn publish(
    store: &PackedStore,
    input: &Rows,
    records: &[RecordRef],
    range: Range<usize>,
) -> Result<u64> {
    let mut update = store.begin_append()?;
    let mut values = Vec::new();
    for start in (range.start..range.end).step_by(SEGMENT_ROWS) {
        let end = range.end.min(start + SEGMENT_ROWS);
        values.resize((end - start) * input.dimensions, 0.0);
        for (index, row) in values.chunks_exact_mut(input.dimensions).enumerate() {
            input.fill(start + index, row);
        }
        let batch: Vec<_> = records[start..end]
            .iter()
            .zip(values.chunks_exact(input.dimensions))
            .map(|(record, vector)| PackedRecord {
                id: &record.id,
                source: &record.source,
                vector,
            })
            .collect();
        update.stage(&batch)?;
    }
    update.commit()
}

fn compare_search(
    v1: &Snapshot,
    packed: &PackedGeneration,
    queries: &[Vec<f32>],
) -> Result<(Value, Value)> {
    let baseline = |query: &[f32]| search(v1, query, TOP_K);
    let candidate = |query: &[f32]| packed.search(query, TOP_K);
    assert_eq!(
        baseline(&queries[0])?,
        candidate(&queries[0])?,
        "warmup parity"
    );

    let mut v1_samples = Vec::new();
    let mut packed_samples = Vec::new();
    for (index, query) in queries.iter().enumerate() {
        let (left, right) = if index % 2 == 0 {
            (
                timed_search(|| baseline(query))?,
                timed_search(|| candidate(query))?,
            )
        } else {
            let right = timed_search(|| candidate(query))?;
            (timed_search(|| baseline(query))?, right)
        };
        assert_eq!(left.0, right.0, "top-K results differ for query {index}");
        v1_samples.push(left.1);
        packed_samples.push(right.1);
    }
    let v1_qps = concurrent_qps(queries, baseline)?;
    let packed_qps = concurrent_qps(queries, candidate)?;
    Ok((
        statistics(v1_samples, v1_qps),
        statistics(packed_samples, packed_qps),
    ))
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn timed_search(run: impl FnOnce() -> Result<Vec<SearchHit>>) -> Result<(Vec<SearchHit>, f64)> {
    let started = Instant::now();
    let hits = run()?;
    Ok((hits, elapsed_ms(started)))
}

fn concurrent_qps(
    queries: &[Vec<f32>],
    run: impl Fn(&[f32]) -> Result<Vec<SearchHit>> + Sync,
) -> Result<f64> {
    let started = Instant::now();
    for batch in queries.chunks(CONCURRENT_QUERIES) {
        batch.par_iter().try_for_each(|query| -> Result<()> {
            black_box(run(query)?);
            Ok(())
        })?;
    }
    Ok(queries.len() as f64 / started.elapsed().as_secs_f64())
}

fn statistics(mut samples: Vec<f64>, concurrent_qps: f64) -> Value {
    samples.sort_by(f64::total_cmp);
    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    let qps = 1000.0 * samples.len() as f64 / samples.iter().sum::<f64>();
    json!({
        "p50_ms": percentile(50),
        "p95_ms": percentile(95),
        "serial_qps": qps,
        "bounded_concurrent_qps": concurrent_qps,
        "sorted_samples_ms": samples,
    })
}
