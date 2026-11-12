# Exact-Search and Storage Architecture: Source Audit (2026-11-11)

Parent index: [Lodestone Planning](INDEX.md)

## Status and decision boundary

**Desk research and source audit; no Lodestone or competitor performance measurements were run.** This is a corrected, narrower synthesis of the Deep Research survey, emphasizing **exact nearest-neighbor search and compact, embedded, file-native architectures**. It is not a claim of performance superiority and does not select ANN. The companion [v2 design proposal](v2-canonical-segments-design-2026-11-11.md) is a **proposal**, while [ADR-0003](../decisions/0003-canonical-packed-segments-and-derived-indexes.md) accepts only the direction.

Source classifications used below:
- **Primary implementation/documentation:** official project source or format documentation; establishes capabilities/layout, **not** a cross-project performance result.
- **Research:** published paper; conclusions apply to its tested workload/hardware, not automatically to Lodestone.
- **Unverified third-party/vendor benchmark:** illustrative only; not evidence of Lodestone latency, comparative speed, or a universal threshold.
- **Derived:** arithmetic or engineering hypothesis; not measured.

## Corrections to the initial Deep Research report

| Report statement | Audit and disposition |
| --- | --- |
| “10M x 768 f32 vectors use ~2.86 GB” | **Incorrect by 10x.** `10,000,000 * 768 * 4 = 30,720,000,000` bytes = **30.72 GB = 28.61 GiB**. **1M** x 768 is 3.072 GB = 2.861 GiB. |
| “FAISS Flat 10M x 768: 44.7 ms p95” | Reported by a [third-party comparison](https://markaicode.com/benchmarks/faiss-production-benchmark-latency/), **not verified as reproducible primary evidence**. Reading 30.72 GB in 44.7 ms implies **~687 GB/s effective input bandwidth** if all bytes are streamed; this requires a documented hardware/cache/concurrency explanation. **Exclude from design targets.** |
| “FAISS Flat 10M uses 2.86 GB” | Conflicts with the official [FAISS index table](https://github.com/facebookresearch/faiss/wiki/Faiss-indexes): Flat f32 costs **4d bytes/vector**, or 30.72 GB for 10M x 768, before auxiliary memory. Exclude the third-party footprint claim. |
| “1M scan takes 30 ms at 128 GB/s” | Arithmetic is **24 ms**: 3.072 GB / 128 GB/s = 0.024 s. 128 GB/s is an **illustrative assumed sustained bandwidth**, not measured Lodestone hardware. |
| “A few tens of millions of floats/sec” at 128 GB/s | Unit error: 128 GB/s / 4 B = **32 billion floats/sec**, assuming that rate is achievable. |
| “FAISS/SQLite/Lodestone match in performance” | **Unsupported.** No controlled, same-hardware, same-metric Lodestone-versus-FAISS Flat experiment exists in the reviewed evidence. |
| “ANN necessary above 0.5–1M vectors” | **Not universal.** Exact-search acceptability depends on query rate, concurrency, dimension, memory hierarchy, hardware, selectivity and latency budget. No fixed row-count cutoff adopted. |
| “LanceDB 25 ms at 1M” and “Qdrant highest throughput” | Vendor benchmarks with differing search/index configurations; **do not classify as exact flat scans** or compare directly to Lodestone. |
| “CAS deduplicates identical vectors automatically” | CAS deduplicates identical **keys/objects**, depending on hash domain. **Lodestone v1 address is based on format/model/source identity**, not simply vector bytes; equal numeric vectors with distinct source identities need not share an object. Segment-level packing alone does not imply cross-identity vector deduplication. |
| “Content hash makes corruption impossible” | Hash validation detects mismatches within the chosen threat model; durability, torn writes, collisions, metadata consistency and verification policy still need design. |
| “No duplicate payloads under all conditions” | Target is **one canonical full-precision copy per live identity in steady state**; compaction, snapshots, backups, readers and rollback can transiently retain copies. |

The 2026 ANN papers and vendor results in the original report are context for potential future acceleration, not a performance basis for exact search.

## Evidence matrix: comparable architectures

| Reference | Supported primary-source fact | Applicability | Not established |
| --- | --- | --- | --- |
| [FAISS Flat](https://github.com/facebookresearch/faiss/wiki/Faiss-indexes) | `IndexFlatIP` and `IndexFlatL2` are exhaustive, with **4d bytes/vector** for f32; [index guidance](https://github.com/facebookresearch/faiss/wiki/Guidelines-to-choose-an-index) treats Flat as exact baseline | Closest algorithmic baseline for packed exhaustive search, top-K, batching | Lodestone-vs-FAISS latency or persistence equivalence |
| [FAISS library](https://github.com/facebookresearch/faiss) | Supports dot-product/L2 and cosine via normalization; distinguishes compact codes and graph overhead | Search kernel design and separation of index representations | File-native durable transaction semantics |
| [sqlite-vec](https://github.com/asg017/sqlite-vec) | SQLite extension offering vector search; inspect version-specific implementation before comparing | Embedded distribution, SQL/row access and extension overhead | Specific speedup, mmap layout or equal-footprint claims |
| [SQLite-Vector](https://github.com/sqliteai/sqlite-vector) | Separate SQLite extension project; do **not** conflate with sqlite-vec | Embedded exact-scan design alternative | Claims from secondary articles without reproducible harness |
| [Lance format](https://lance.org/format/) | Separates data files, table fragments/manifests/deletions, and index formats | Strong reference for immutable files, versioning, logical deletion and index independence | LanceDB Enterprise ANN benchmark as exact NN result |
| [Lance format versioning](https://lance.org/format/file/versioning/) | Explicit file/encoding version contracts; unstable versions may break compatibility | Supports explicit v2 identity/reader strategy | Compatibility of Lodestone v1 files |
| [pgvector](https://github.com/pgvector/pgvector) | PostgreSQL extension offers exact search by default and optional HNSW/IVFFlat | SQL executor, row-store and durability comparison; baseline must be **exact vs exact** | Third-party 600 ms figure as universal pgvector latency |
| [Qdrant storage](https://qdrant.tech/documentation/manage-data/storage/) | Documents segments, mmap and WAL | Segmented storage and recovery patterns | Qdrant ANN benchmark as an exact-scan comparison |
| [DuckDB VSS](https://github.com/duckdb/duckdb-vss) | Experimental vector indexing extension using USearch/HNSW | Example of derived index atop general-purpose storage | Exact DuckDB scan speed from HNSW numbers |
| [LSM-tree paper (O'Neil et al., 1996)](https://db.cs.berkeley.edu/cs286/papers/lsm-acta1996.pdf) | Buffered writes, immutable/merged disk-oriented components as a storage design family | Flush/merge lifecycle and write amplification vocabulary | Proof that a specific Lodestone segment size/compaction policy is optimal |

**Source quality rule:** official project docs establish semantics; a third-party benchmark establishes only its claimed observation under its own conditions. Paper, vendor and community benchmark values cannot be pooled into a single speed ranking without workload/hardware/recall normalization.

## Lodestone v1 analytical baseline, independently checked

Code ownership: `crates/core/src/object_format.rs`, `object.rs`, `format.rs`, `materialize_format.rs`, `search.rs`; details in the [earlier investigation](storage-amplification-and-scale-2026-11-11.md).

For one model/dimension set: `N` active rows, `U` unique v1 object identities, `D` f32 dimensions, model identity length `M`, per-object source identity lengths `S_i`, record-ID lengths `R_i`:

- Raw active vectors: `4DN`.
- Object **logical file bytes**: `sum_U (24 + M + S_i + 4D)`.
- Snapshot logical bytes: `align64(align8(64 + M) + 48N + sum_N R_i) + 4DN`.
- Total: object bytes + snapshot bytes + any other generations, metadata, temp files and filesystem allocation overhead.

For `N=U=1M`, `D=768`, `M=16`, `S_i=64`, `R_i=24`: objects **3,176,000,000 B**; one snapshot **3,144,000,128 B**; combined **6,320,000,128 B**, or **2.0573x** ideal active f32 payload. This is **logical file-length arithmetic**, not measured physical allocated bytes.

## Bandwidth and exact-search model

For an unfiltered exhaustive f32 scan of `N` rows and `D` dimensions, vector bytes read are approximately `4ND` when not served from a higher cache level. A bandwidth-only lower bound is `T >= 4ND / B_sustained`; actual time also includes arithmetic, top-K selection, metadata/filter checks, thread coordination, mapping/page faults and output.

| N, D=768 | f32 payload | Ideal time at assumed 32 GB/s | At assumed 128 GB/s |
| --- | ---: | ---: | ---: |
| 100K | 0.3072 GB | 9.6 ms | 2.4 ms |
| 1M | 3.072 GB | 96 ms | 24 ms |
| 10M | 30.72 GB | 960 ms | 240 ms |

These are **scenario calculations**, not predictions or benchmark measurements. The bandwidth available to one process may be much lower; warmed data may be served from CPU cache, and mmap over nonresident data may be storage-I/O-bound. Multiple queries can share loaded vectors if batched, but concurrency can saturate the same memory channels. Query distribution affects candidate pruning but generally not how many f32 bytes a full exact scan must visit.

**Exact search remains the default candidate**: preserve full-corpus correctness and deterministic top-K, with optimized contiguous scans, optional parallelism and optional batch query reuse. An index that only selects a subset is approximate **unless** it supplies a mathematically sound bound proving that omitted vectors cannot improve top-K. High-dimensional bounds may prune poorly; do not assume sublinear exact search.

## Findings and limits

1. Lodestone's **largest demonstrated footprint issue is duplicated persistent vector payload**, not an established weakness of its exact search kernel.
2. A packed exact-scan representation is a credible design: FAISS Flat provides a well-documented algorithmic reference. It does **not** establish Lodestone speed relative to FAISS.
3. Segmenting canonical storage is primarily a **durability, locality and incremental update** choice. Exact search across all live segments still examines all eligible vectors; segment count adds dispatch/merge overhead.
4. Store identity and location must be separated. Lodestone's existing model/source-derived address contract cannot silently be replaced by a hash of f32 bytes.
5. Avoid benchmarking ANN systems as if their low latencies were exact-search results. Do not select HNSW, IVF, quantization or arbitrary fixed vector-count thresholds here.

## Minimal validation after design review

- **Storage:** small deterministic corpus with repeats, deletions and two generations; compare apparent and allocated bytes, steady-state vs compaction peak, v1 vs proposed v2.
- **Exactness:** independent scalar dot-product reference, ties/NaN rejection, filters, tombstones, generation switches, top-K ordering and cross-segment merging.
- **Kernel:** 100K and 1M rows at 384/768 dimensions, warm/cold, 1-thread/multi-thread, top-K 1/10, measured memory bandwidth, CPU, page faults and RSS.
- **Reference:** optional same-machine FAISS `IndexFlatIP` exact baseline, same vectors/metric/normalization and concurrency; measure only if a design decision depends on it.
- **Durability:** fail at every publication stage, validate restart/rollback/GC behavior on Windows and Unix.

No broad ANN benchmark campaign is required for the first v2 milestone. Document hardware, versions and reproducibility before interpreting any measured difference.

## Follow-up

See [v2 canonical segments design proposal](v2-canonical-segments-design-2026-11-11.md), [ADR-0003](../decisions/0003-canonical-packed-segments-and-derived-indexes.md), and [roadmap](roadmap.md). This audit does **not** amend the implemented v1 contracts in [Storage](../STORAGE.md) or [Snapshots](../SNAPSHOTS.md).
