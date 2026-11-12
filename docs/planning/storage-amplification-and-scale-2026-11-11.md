# Storage Amplification and Scale Investigation (2026-11-11)

Parent index: [Lodestone Planning](INDEX.md)

## Status and scope

**Source-inspected analytical assessment, not an empirical cross-engine benchmark.** This records the storage/indexing discussion, derives current on-disk byte counts from the v1 implementation, and defines the measurements required to substantiate competitive claims. No comparative pgvector, Qdrant, or LanceDB disk-usage measurements have been performed for this report. It is not an ANN implementation decision.

## Design thesis and constraints

Lodestone's prospective advantage is a small embedded deployment and low **total persistent storage footprint**, rather than an invented vector similarity algorithm. Floating-point embedding payloads do not generally yield large *lossless* compression savings; eliminate storage/indexing overhead before introducing lossy representations. Preserve content-addressed immutable authority, validated memory-mapped reads, narrow FFI, and no required database service. However, mmap avoids duplicate **process heap** materialization; it does not reduce the number of persistent vector copies. Nor does a packed contiguous representation imply that the complete object store is packed once.

Current implementation:

- `crates/core/src/object_format.rs`: each immutable object is a 24-byte header, model bytes, source bytes, then `4 * dimensions` vector bytes.
- `crates/core/src/object.rs`: one individual object file per unique (model, source) address under `objects/<2-hex>/<62-hex>`; repeated references to an identical object need not produce duplicate object files. File metadata, filesystem blocks and directory entries add physical overhead not counted here.
- `crates/core/src/format.rs` and `materialize_format.rs`: one snapshot has a 64-byte header, model bytes, alignment padding, 48 bytes per record entry (including a 32-byte object hash), concatenated record-ID bytes, padding to 64 bytes, and a second full-precision `f32` matrix.
- `crates/packed/src/lib.rs`: a generic dense fixed-width row substrate supporting several scalar tags, **not** a deployed quantized persisted index. V1 snapshot payload is still `f32`.
- `crates/core/src/search.rs`: exact dot-product scan (serial or Rayon-parallel); no ANN, IVF, PQ or incremental search index in this reviewed path.
- Snapshot generation requires all referenced objects and rebuilds a complete snapshot. Existing-target replacement has documented availability risks; see [Snapshots](../SNAPSHOTS.md).

## Reproducible analytical byte model

Let `N` be active snapshot rows, `U` unique immutable vector objects, `D` dimensions, `M` model identity UTF-8 byte length, `S_i` source identity byte length, and `R_i` record-ID byte length.

- Raw ideal float32 vector payload per active row: `4DN`.
- Object **file lengths**: `sum over U of (24 + M + S_i + 4D)`. This excludes filesystem allocation/metadata. Different object-model or dimension combinations require separate accounting.
- Snapshot **file length**: `align64(align8(64 + M) + 48N + sum(R_i)) + 4DN`.
- One-snapshot total logical file bytes = sum of object file lengths + snapshot length; add other snapshots, manifests if stored, temp files, and physical filesystem overhead separately.
- Storage amplification = total measured allocated bytes (or explicitly identified logical file bytes) divided by `4DN`. Report which definition is used. Retaining historic objects can make `U > N`; reference reuse may make `U < N`.

**Illustrative analytical scenario (not a measurement):** `N=U=1,000,000`, `D=768`, `M=16` UTF-8 bytes, `S_i=64` bytes, `R_i=24` bytes, one active v1 snapshot, no old generations:

| Component | Logical bytes | GiB |
| --- | ---: | ---: |
| Ideal raw vectors (`4DN`) | 3,072,000,000 | 2.861 |
| Immutable object files | 3,176,000,000 | 2.958 |
| Materialized snapshot | 3,144,000,128 | 2.928 |
| Combined (one generation) | 6,320,000,128 | 5.886 |
| Combined / ideal vectors | **2.0573x** | — |

The two full vector payload copies dominate this case. Small files can materially worsen **allocated** disk usage; generation retention and write amplification can worsen peak footprint. Conversely, object reuse and ID lengths affect the results. It is inaccurate to compare the single snapshot's near-ideal footprint directly to a complete Postgres or Qdrant deployment.

## Competitive comparison: documented properties, not measured winners

| System | Verified documented storage/index property | What remains unmeasured |
| --- | --- | --- |
| Lodestone v1 | Immutable object files + full duplicate packed `f32` snapshot; exact scan and mmap; per-record 48-byte snapshot entry | Actual allocated disk bytes, ingest/refresh peak, latency, RSS/page cache across workloads |
| pgvector | PostgreSQL `vector` value is `4D + 8` bytes *before* PostgreSQL tuple/page, table/index/WAL, and application schema overhead; exact scan, HNSW and IVFFlat available | Total table + indexes + WAL physical bytes for same records, configs and durability |
| Qdrant | Mmap-backed vector storage with segments, ID mapping, payload/index structures, WAL; HNSW and optional quantization/index memory tiers | Physical collection and WAL footprint for same payload/filter/index profile |
| LanceDB | Columnar/file-oriented and indexed alternatives make it a relevant comparison target | A pinned implementation/configuration, source-backed byte model and controlled measurements |

Sources (retrieved 2026-11-11):
- pgvector README: https://github.com/pgvector/pgvector/blob/master/README.md (vector physical type bytes, exact/HNSW/IVFFlat).
- Qdrant storage: https://qdrant.tech/documentation/manage-data/storage/ (segments, memory-mapped vectors, WAL).
- Qdrant indexing: https://qdrant.tech/documentation/manage-data/indexing/ (payload-filter indexes and planner).
- Qdrant memory tiers: https://qdrant.tech/documentation/ops-configuration/memory-tiers/ (index/quantization tradeoffs).
- Internal code references above are authoritative for this revision; competitor descriptions are capabilities, **not benchmark numbers**.

Do not claim Lodestone beats these alternatives in total bytes, ingestion, query throughput, or memory until controlled results exist. Pgvector is common, but not a proven majority of all deployed vector storage.

## Main conclusions

1. **Footprint hypothesis requires refinement:** the current snapshot itself is close to its raw `f32` matrix baseline, but the full system has approximately **2x vector payload** with one unique object per row and one snapshot. Removing duplicate vector data could be a bigger footprint gain than making the 48-byte entry smaller.
2. **ANN is a scale capability, not a free upgrade:** HNSW graphs consume memory/disk; IVF-Flat may offer leaner indexing; PQ/quantization can reduce search working set at a recall cost. Do not declare a universal winner before testing recall, update cost, and filter behavior.
3. **Incremental updates matter:** full snapshot rematerialization is expensive for continuously growing memory. Consider immutable segments + a bounded mutable/delta layer, generation manifests, compaction, tombstones and stable query-time merging, while avoiding an unbounded pile-up of generations.
4. **Authority vs acceleration:** preserve content-addressed authoritative objects. Make ANN structures validated, versioned, rebuildable derived artifacts; investigate an **offset/reference-based** searchable view before introducing further full-vector copies. Any zero-copy alternative must prove file lifetime, crash recovery, random access locality, and content-integrity guarantees.
5. **Filter-aware ANN:** Reliquary's identity/tenant/memory-scope boundaries must be enforced *before or within* retrieval, not solely after globally selecting top-K. Scope cardinality should influence exact-vs-ANN planning. Lodestone should expose generic filters/partitions rather than absorb application memory policy.
6. **Deployment profiles:** exact scan for small or highly filtered sets; ANN for recall/latency-bound collections; compact on-disk indexes for capacity-bound use. Thresholds require measurements, not guessed row counts.

## Measurement protocol and acceptance gates

Create an **automated and reproducible** corpus/harness; record commit hashes, OS, filesystem/block allocation, CPU, RAM, storage, compiler profile, vector model/dimensionality/distribution, seeds, IDs, snapshots kept, durable ingest settings, filter selectivity and tool versions.

- Scales: 10K, 100K, 1M, 10M rows; 384/768/1536 dimensions, realistic corpus plus seeded synthetic vectors. For every scale capture distinct object count versus active row count and source/record-ID lengths.
- **Physical footprint:** recursively measure apparent file length and allocated blocks; distinguish authoritative objects, active snapshots, old generations, temp, metadata, external application manifests, indexes and WAL. Record peak during ingestion, indexing, compaction and failed publication.
- **Search:** p50/p95/p99 latency (cold and warmed cache), QPS, exact ground truth, recall@10/@100, top-K quality, RSS, mapped bytes, page faults and CPU. Include filtered queries at multiple scope selectivities and verify isolation.
- **Mutations:** sustained inserts, replacements, tombstones, incremental index catch-up, full rebuild duration, write amplification and crash-restart recovery.
- **Competitors:** same vector data, IDs, metric, payload projection, durability, concurrency, memory cap, and exact-vs-approximate recall target. Compare pgvector plain table/exact and HNSW/IVFFlat; Qdrant mmap/exact and indexed/quantized; LanceDB exact/indexed after verifying supported configurations. Report versions and all tuning.
- **Gate:** do not select HNSW, IVF/PQ, segment format, or a default planner solely from analytical footprint. Demand actual disk-to-raw ratio, recall/latency tradeoff and update/peak-disk cost at 100K and 1M before promoting a design; 10M is a further scaling gate.
- **Compatibility:** v1 object addresses and snapshot bytes are frozen; any new persisted type/format has an explicit identity/version, conversion plan, fixtures, corruption testing and atomic-generation publication contract.

## Candidate implementation sequence (proposal only)

1. Build reproducible footprint/search/ingest harness and publish raw results. Measure **existing** Lodestone baseline, including multiple generations and small-file block overhead.
2. Prototype two non-mutating derived index families with identical test corpus: HNSW and IVF-Flat, plus exact selective-scope baseline. Track disk/RAM amplification, filtered recall and build/update costs.
3. Evaluate segmented immutable search generations and bounded delta index; establish manifest, tombstone, compaction and atomic switch behavior with crash/FFI tests.
4. Investigate vector-storage deduplication (single canonical vector payload with indirect search addressing or compact immutable segment objects). Preserve content identity and check mmap/random-access penalties.
5. Consider f16/int8/PQ representations **only** as explicitly new indexed forms with full-precision rerank and measured retrieval quality; no silent v1 rewrite.

## Relationship to existing plans

This is research and prioritization, not a replacement for [current roadmap](roadmap.md): generation-safe publication, v1 compatibility fixtures, and cross-platform correctness are prerequisites for introducing new persisted index formats. Do not present any candidate as implemented.
