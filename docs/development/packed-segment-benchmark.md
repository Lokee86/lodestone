# Packed-segment benchmark

Parent index: [Lodestone Development Documentation](INDEX.md)

## Purpose

Measure the effect of storing one full-precision vector payload per identity while preserving Lodestone's exact search results. Pin the v1 bytes and behavior that a future migration must preserve. [V2 design proposal](../planning/v2-canonical-segments-design-2026-11-11.md)

## Overview

The Rust benchmark runs as an ignored release test on little-endian Windows and Unix hosts. It stages inputs with the packed store, commits a generation, and opens a fresh view through the same Rust API used by consumers. Each benchmark record has its own model/source identity. Both layouts use the current exact dot-product kernel. The benchmark compares IDs, scores, and ordinals for every timed query.

The publication tests kill a writer at each boundary, search an already mapped view, and verify recovery by opening a fresh view. [Benchmark implementation](../../crates/core/src/benchmark/mod.rs) · [Publication tests](../../crates/core/src/packed_store/tests.rs)

## Reproduce

Use a freshly created local scratch directory or the system temporary directory. Set LODESTONE_BENCH_SCRATCH to choose the volume. The runner creates and removes its own temporary directory. It emits one JSON record prefixed with LODESTONE_BENCHMARK, including the sample latencies and file totals.

Run the seeded 100K search comparison from PowerShell. Set LODESTONE_BENCH_ROWS to 1000000 for the larger tier.

~~~powershell
$env:LODESTONE_BENCH_INPUT = $null
$env:LODESTONE_BENCH_ROWS = '100000'
$env:RAYON_NUM_THREADS = '8'
cargo test -p lodestone-core --release --locked --lib benchmark::benchmark_packed_segments -- --exact --ignored --nocapture
~~~

Set LODESTONE_BENCH_INPUT to a flat little-endian float32 matrix with 1,024 dimensions and one compatible embedding profile. The real-data mode uses the last 20 rows as queries and appends up to 128 of the remaining rows after its initial build. It calls the normal v1 object writer and snapshot materializer. The seeded mode uses the same v1 snapshot encoding with rows from a fixed seed and measures the two search layouts without materializing the per-object store.

Set LODESTONE_BENCH_COMPACTION_ROWS to 32768 to repack the published generation after the v1 comparison. The benchmark checks every query against the original results in both the reopened compacted generation and the held-open original reader. It runs reclamation with that reader open and again after closing it, then queries a fresh view. The JSON output includes live segment counts, allocated bytes with both segment sets retained, and remaining allocation after reclamation.

Run the v1 compatibility tests through the normal workspace test command. The two frozen objects and the snapshot were emitted by v1 at commit a4d6443a73df26d5aa5696af884d4e356c7f458f. The fixture has unsorted input, two record IDs sharing one object, a negative score, and tied first-place results. The tests verify the old bytes and compare them with output from the current writer. [V1 fixture tests](../../crates/core/tests/v1_compatibility.rs)

## Windows prototype baseline on November 20, 2026

The original run used an experimental segment layout with v1 snapshot encoding. It ran on Windows 11 build 26300, an i9-11900H with eight physical cores and 32 GB RAM, NTFS with 4 KiB allocation units, and release Rust 1.100.0 with eight Rayon workers. It timed 20 queries after opening and warming both layouts, alternating the layout measured first. Throughput uses batches of four concurrent queries. These are single-host measurements.

The real dataset was the checksum-verified Archive matrix from one active profile in the 28-day corpus. It had 2,882 rows at 1,024 dimensions, including the 20 held-out queries. All 60 top-10 comparisons agreed across the three workloads.

| Searchable rows | Data | V1 warm p95 | Packed warm p95 | V1 queries/s at concurrency 4 | Packed queries/s at concurrency 4 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 2,862 | 28-day Archive | 0.70 ms | 0.79 ms | 2,069 | 2,097 |
| 100,000 | Seeded | 16.70 ms | 18.82 ms | 65.1 | 63.3 |
| 1,000,000 | Seeded | 193.18 ms | 210.52 ms | 5.34 | 5.04 |

The real-data baseline used 33.71 MiB of allocated file space across 2,863 files. The packed layout used 11.36 MiB across four files, including the two retained manifests. That is a 66.3% reduction. The 128-row append emitted 11.86 MiB in v1 and 0.51 MiB in the packed layout, about 23.3 times fewer output bytes. Output counts include serialized objects, snapshots, segments, or manifests as applicable. Allocation counts cover regular files and their data blocks.

## Published-store rerun on November 20, 2026

The rerun used the same host, toolchain, inputs, held-out queries, and concurrency. It measured reopened generation 2 on the real dataset and generation 1 on the seeded datasets. All 60 top-10 comparisons agreed on IDs, scores, and order.

| Searchable rows | Data | V1 warm p95 | Published packed warm p95 | V1 queries/s at concurrency 4 | Published packed queries/s at concurrency 4 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 2,862 | 28-day Archive | 0.72 ms | 0.67 ms | 2,058 | 1,566 |
| 100,000 | Seeded | 17.55 ms | 19.11 ms | 62.0 | 63.3 |
| 1,000,000 | Seeded | 194.72 ms | 197.91 ms | 6.09 | 6.11 |

On the real dataset the published store allocated 11.38 MiB across five files, including the profile and both manifests. V1 allocated 33.71 MiB. The 128-row append emitted 0.51 MiB in the packed store and 11.86 MiB in v1. The seeded tiers measure search, using a v1-format image built directly from the same rows.

## Compaction verification on November 20, 2026

With a target of 32,768 source rows per replacement segment, compaction used the regular packed-store commit path. The 20 held-out queries at each size returned identical top-10 IDs, scores, and order before compaction, after reopening the compacted generation, and from the original reader held open across publication.

| Searchable rows | Data | Live segments before | Live segments after | Compaction elapsed | Allocated space with both segment sets retained |
| ---: | --- | ---: | ---: | ---: | ---: |
| 2,862 | 28-day Archive | 2 | 1 | 65.7 ms | 22.75 MiB |
| 100,000 | Seeded | 7 | 4 | 1.72 s | 794.64 MiB |
| 1,000,000 | Seeded | 62 | 31 | 19.69 s | 7.76 GiB |

Elapsed time covers reading the source generation, writing and validating the replacement segments, and publishing the manifest. The allocated space column also includes the profile and retained manifests. [Compaction contract](../PACKED_STORE.md#compaction)

## Reclamation verification on November 20, 2026

The Windows rerun used the same 2,862 searchable Archive rows. Cleanup removed zero segments while the original view was pinned. After it closed, cleanup removed two obsolete segments and reduced allocated file space from 22.75 MiB to 11.37 MiB across the profile, current manifest, and one compacted segment. All 20 held-out queries returned the same top-10 IDs, scores, and order from the pinned and compacted views and after reopening following cleanup. [Reclamation contract](../PACKED_STORE.md#reclamation)

## Comparison with FAISS, LanceDB, Qdrant, and pgvector on November 20, 2026

The same Windows 11 NTFS host ran CPU searches with 1,024-dimensional float32 vectors, inner product, and top 10. The workloads were the checksum-verified 28-day Archive fixture and the 100,000-row uniform-random fixture generated from Lodestone's existing seed. Each mode warmed up the same 20 held-out queries and timed five serial rounds. The PostgreSQL and Qdrant timings include the localhost client call. Each reported recall is measured against an independent exhaustive float64 pass. When scores tie at the cutoff, any tied ID can fill a remaining top-10 slot. Six Archive queries tied at the cutoff.

| Engine and search mode | Archive p95 | Archive allocated MiB | 100K p95 | 100K allocated MiB | 100K recall@10 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lodestone packed exact | 0.839 ms | 11.37 | 16.084 ms | 397.32 | 100% |
| FAISS FlatIP exact | 1.483 ms | 11.18 | 27.985 ms | 390.63 | 100% |
| FAISS HNSWFlat | 0.099 ms | 11.57 | 10.293 ms | 404.39 | 44.5% |
| LanceDB exact | 19.383 ms | 11.19 | 271.588 ms | 390.82 | 100% |
| LanceDB IVF_FLAT | 8.034 ms | 22.51 | 31.780 ms | 782.16 | 88.5% |
| Qdrant HNSW | 0.743 ms | 222.84 | 16.398 ms | 467.08 | 49.0% |
| pgvector exact | 10.998 ms | 15.45 | 348.513 ms | 534.87 | 100% |
| pgvector HNSW | 0.774 ms | 33.51 | 152.509 ms | 1,316.18 | 49.5% |

The Archive rows show the fastest tested setting at 100% observed recall. The 100K rows show the highest observed recall per mode. FAISS 1.16.0, Qdrant 1.20.2, and pgvector 0.8.7 on PostgreSQL 18.7 used full-precision HNSW with M=16 and efConstruction=100, and swept search ef from 32 to 1,000. The displayed HNSW points use ef=32 on Archive and ef=1,000 at 100K. LanceDB 0.38.0 used 16 of 32 partitions on Archive and 96 of 128 at 100K. Query plans and indexed row counts confirmed index use. The faster of the serial and parallel PostgreSQL plans is shown for each exact workload. [FAISS index reference](https://github.com/facebookresearch/faiss/wiki/Faiss-indexes) · [LanceDB Python API](https://lancedb.github.io/lancedb/python/python/) · [Qdrant indexing](https://qdrant.tech/documentation/manage-data/indexing/) · [pgvector index reference](https://github.com/pgvector/pgvector#hnsw)

Allocated bytes cover each dataset's persisted files. FAISS reports its serialized index. PostgreSQL includes the table, TOAST, primary key, and HNSW where present. Qdrant used on-disk vectors and graph files with its default payload storage. Its Archive allocation includes preallocated segment files observed after reopening and resuming the optimizer. WAL is accounted for separately. It occupied 32 MiB and 464 MiB for the PostgreSQL cluster and 64 MiB and 96 MiB for the Qdrant collection on Archive and 100K respectively.

Lodestone's exact scan stayed below 1 ms p95 on the measured Archive workload and used about 1.7% more space than the raw vectors at both sizes. The ANN recall figures at 100K characterize uniform-random vectors under the tested settings. Archive recall describes the 20 held-out queries.

## Related docs

- [Packed store and generation publication](../PACKED_STORE.md)
- [Behavioral-contract matrix](../BEHAVIORAL_CONTRACT_MATRIX.md)
- [Storage-amplification investigation](../planning/storage-amplification-and-scale-2026-11-11.md)

## Notes

Search viability at larger sizes depends on the workload's latency budget and concurrency. These measurements provide a warm-search baseline for that decision. Cold-cache behavior and sustained ingest are additional measurements for a production candidate. [Exact-search audit](../planning/exact-search-source-audit-2026-11-11.md)
