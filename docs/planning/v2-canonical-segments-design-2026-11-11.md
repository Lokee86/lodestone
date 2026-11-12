# Lodestone v2: Canonical Packed Segments (Design Proposal, 2026-11-11)

Parent index: [Lodestone Planning](INDEX.md)

**Status: Proposed, not implemented or accepted as a binary/ABI contract.** [ADR-0003](../decisions/0003-canonical-packed-segments-and-derived-indexes.md) accepts the *direction* only. This document makes the architecture reviewable and identifies decisions that require prototypes or follow-up ADRs. Existing [Storage](../STORAGE.md), [Snapshots](../SNAPSHOTS.md), [Architecture](../ARCHITECTURE.md) and [Operations](../OPERATIONS.md) remain the authoritative **v1** behavior.

## Goals and non-goals

**Goals:** one authoritative full-precision payload per retained vector identity in normal steady state; contiguous mmap-friendly exact search; preserved v1 model/source identity semantics; deterministic top-K; bounded incremental writes; crash-safe generation publication; versioned, independently rebuildable search acceleration; cross-platform handle safety; explicit migration/rollback.

**Non-goals:** mandatory ANN, a server/daemon, embedding generation, application memory/tenant policy, transparent v1 format reinterpretation, guaranteed 1.0x total physical bytes (metadata and retained generations exist), or guaranteed zero-copy across all compaction and migration operations.

## Identity model: distinguish four things

1. **Logical record ID**: application-supplied key identifying a *reference* in a generation. Many records may reference the same authoritative object. Update/delete visibility is by record ID and generation.
2. **v1 object address**: existing BLAKE3 domain over v1 format/model/source identity; **not simply a hash of vector bytes**. Keep its exact bytes and conflict rule for v1. A different vector at the same model/source identity remains an error under that identity contract.
3. **v2 authoritative vector identity**: propose a versioned semantic key preserving the model/source-derived identity and an explicit digest/validation of the associated numeric payload. Decide whether to retain the v1 address as a compatibility alias or introduce a v2 address domain. Equal vectors under different source identities do not automatically collapse into one identity. Optional payload-only dedup requires a separate indirection layer and cost/semantics review.
4. **Physical locator**: immutable `(segment identity, row ordinal)` or checked byte offset, valid only for that segment; **never** a permanent external identity. Compaction changes locators while semantic identities remain stable.

**Invariant:** a generation resolves each visible record to exactly one validated authoritative identity and its location; no duplicate full f32 matrix is materialized solely for search. Index references must resolve through the generation's locator table, not stale cross-generation offsets.

## Proposed file and metadata structure

```text
store/
  v1/                      # existing objects/snapshots, retained through migration
  v2/
    segments/<hash>.seg     # immutable authoritative packed f32 payload + metadata
    manifests/<hash>.gen    # immutable generation manifest
    indexes/<hash>.*        # optional derived, rebuildable search structures
    current/                # publication selector / durable pointer protocol TBD
    staging/                # incomplete files; never reader-visible
```

This is a **logical namespace sketch**, not a committed path or binary layout.

### Segment contract (candidate)

- Explicit v2 magic, format version, header length, encoding tag (initially canonical little-endian IEEE-754 f32), dimensions, embedding/model identity, row count, offsets/lengths, checksum/hash domain and optional feature flags.
- A compact **row identity table** mapping semantic vector keys to row ordinals; optional record-ID mapping is generation metadata, not necessarily embedded in every segment. Validate unique identities and deterministic ordering.
- One dense vector matrix; matrix base aligned for mapping/SIMD. **Do not assume each row must be 64-byte padded**: row stride `4D` minimizes bytes; choose padding only after testing dimension-specific SIMD alignment and amplification.
- Segment content hash covers canonical encoded bytes and version domain. Segment identity differs from individual vector identity. Validate offsets, overflow, shape, finite numeric values, checksums and identity consistency **before** exposing mappings; choose whole-file vs chunked validation based on measured startup costs.
- Immutability: publish only complete synced files. Segments must be safely readable while newer generations exist. No in-place edits or truncation.

### Generation manifest (candidate)

An immutable, versioned manifest binds: generation ID, predecessor/reference, model identity, dimensions/metric contract, ordered active segment IDs, record-ID to vector-identity/locator references, tombstone/update visibility, index IDs + build-version/coverage, and integrity digests. Store only necessary metadata; avoid per-record duplicated vector bytes.

The **current-generation selector** must be a separate, validated publication mechanism. A single rename over an existing path is **not** assumed atomic on all supported Windows/Unix configurations; use immutable generation names and a platform-verified selector/switch contract with fallback to last valid generation. Preserve a bounded number of pinned generations for live readers and rollback.

### Derived search state

- **Baseline:** exact dot-product scan directly over canonical segment matrices; no second packed search snapshot. Query each live segment, maintain bounded local top-K, merge with stable global ordering and generation-specific visibility. Filter/partition before ranking when applicable. For a filtered scan, ensure all eligible rows are considered.
- **Optimization candidates:** SIMD, parallel segment/row scans, batched queries and safe exact pruning. A skipped region must have a **sound bound** to preserve exactness. An arbitrary centroid-probe limit is ANN, even if selected partitions use exact scans.
- **Optional future indexes:** compact identity/locator metadata or ANN indexes are derived, versioned, validated against a generation and rebuildable. They must not become the sole source of canonical vectors. ANN requires explicit opt-in/recall semantics; not a v2 prerequisite.
- **Search semantics:** preserve current deterministic score/tie/record ordering and finite-query validation; pin a generation for the duration of each search so handle closure/GC cannot invalidate an active mapping. Specify behavior for duplicate record references and multiple models explicitly.

## Incremental write, publication and recovery protocol (proposed)

1. **Intake:** validate vector shape, finite values, model/source identity and address conflict rules; maintain a bounded mutable/delta buffer. Define a durable write-ahead/intake log or equivalent crash-recoverable staging mechanism **before** promising acknowledged-write durability.
2. **Flush:** deterministically encode one or more new immutable segments, write to unique staging files, sync data, verify hashes and structural validity, then publish under immutable content-addressed names. Repeated segment publication is idempotent.
3. **Prepare generation:** write a complete immutable manifest referring only to verified published segments; sync file and required directory metadata as supported by each filesystem.
4. **Switch:** validate manifest and search opening, then advance the current-generation selector via a separately proven cross-platform protocol. The old valid generation remains available until the new one is confirmed.
5. **Read:** open and pin the selected manifest and segments; reject unsupported formats, dangling locators, corrupt checksums, invalid tombstone order or mismatched model/metric.
6. **Compaction:** create replacement segments and manifest without modifying old files; merge live records, resolve tombstones and duplicates deterministically; switch generations after validation. Preserve old segments while any pinned reader/rollback/backup references them.
7. **GC:** mark reachable segments from current, retained and pinned manifests (plus active writers and backup leases), then sweep only unreachable files after a safety window. Do not infer liveness from filenames alone.

### Crash/fault matrix

| Interruption | Required recovery invariant |
| --- | --- |
| During intake or flush | Last acknowledged durable state survives according to documented acknowledgement policy; staging leftovers are ignored/quarantined |
| Segment published, manifest absent | Segment is orphaned but harmless; no reader sees incomplete generation; GC can reclaim later |
| Manifest published, selector not switched | Previous generation remains current; new generation can be retried or reclaimed |
| Selector switch interrupted | Recovery selects a **fully validated** old or new generation, never a partial one |
| Reader open during compaction/GC | Pinned mappings and segment files remain valid; Windows file-handle deletion restrictions respected |
| Corrupt segment or manifest | Reject affected generation; fall back only under explicit recovery policy, never silently serve partial rows |
| Derived index missing/stale/corrupt | Rebuild or use exact scan of canonical segments, without loss of authoritative data |
| Power loss after reported success | Success contract must be backed by file and directory durability tests; unsupported guarantees documented |

**Important:** a manifest checksum does not itself guarantee atomicity, fsync durability or Windows replacement behavior.

## Tombstones, compaction and storage economics

- Tombstones apply to logical **record references**, not automatically to shared vector payloads. An object is reclaimable only when no retained generation, active record, pinned reader or backup requires it.
- An update is a new record-to-identity binding in a new generation, not an in-place vector overwrite. Define source/model address conflicts before accepting replacement embeddings.
- Bound the number of searchable segments and delta layers to control per-query dispatch overhead. Compaction policy should consider tombstone ratio, segment count, size distribution, write amplification and available free disk; **no fixed 25% threshold** is adopted.
- Steady-state target: `4D * U_live + segment metadata + record mapping + manifest/index metadata`, where `U_live` is the number of retained distinct canonical vector identities. It is not automatically `4DN` if many records share identities. Peak storage includes old/new segments, temporary files, retained generations, v1 coexistence and backups.
- Segment-level CAS alone deduplicates **identical segment files**, not necessarily individual vector payloads across different segments. A global identity-to-locator lookup and compaction/dedup policy are required to avoid re-storing the same identity in multiple live segments.

## Compatibility and migration (v1 is immutable)

**Gate 0:** freeze cross-version fixtures for v1 object addresses, object bytes, snapshot magic, ordering, ABI, Windows/Unix behavior and corruption rejection. Never change existing v1 hash domain or silently reinterpret v1 files.

**Candidate migration:** read and validate v1 authoritative objects and application record manifest; write new v2 segments/manifest to a separate namespace; verify each semantic identity and vector bytes against v1; run independent exact top-K comparison (including ties, filters, duplicate references); publish v2 selector only after validation. Preserve v1 data and a dual-read/rollback path until deployment evidence supports retirement.

**Important constraint:** the v1 snapshot is *derived*, and may not contain enough authority to reconstruct all source identity semantics on its own. Do not promise snapshot-only migration without inspecting what the v1 application manifest and object store supply.

Bindings/ABI: new v2 capabilities and handles need explicit negotiation or versioned symbols. A v1-only consumer must reject unsupported v2 state rather than open it as v1. Never invalidate an active mmap through handle closure or GC.

## Open design questions requiring focused decisions

| Question | Preferred starting hypothesis | Evidence needed |
| --- | --- | --- |
| Address domain and dedup | Preserve v1 semantic identities via explicit alias; v2 segment identity is independent | Compatibility fixtures, collision/conflict semantics, duplicate reference workloads |
| Segment sizing | Bounded fixed target with size-aware compaction | mmap overhead, search locality, write amplification, peak disk |
| Row stride/alignment | Align matrix start; initially store dense `4D` stride | SIMD kernels across 384/768/1536 and odd dimensions |
| Locator resolution | Generation-scoped identity -> segment/ordinal mapping | Memory cost, lookup locality, compaction correctness |
| Manifest representation | Deterministic binary or canonical encoding, checksummed and versioned | Reader validation, cross-language stability, forward compatibility |
| Selector publication | Immutable manifest + platform-specific validated switch | Windows/Unix rename and fsync fault tests |
| Intake durability | Bounded delta with explicit durable acknowledgement contract | Crash/restart tests and application write expectations |
| Validation granularity | Full-file validation at publish; cached/chunk checks on open | Startup cost vs corruption detection |
| Filtering | Generic record partition/filter input with exact semantics | Scope isolation, high-selectivity scan cost |
| Retention/GC | Explicit pin/lease and generation reachability | Windows deletion semantics, backup/rollback policy |

## Proposed sequence and acceptance criteria

1. **Documentation and fixtures first:** finalize identity, visibility, durable-acknowledgement and selector invariants; add frozen v1 compatibility fixtures and fault matrix. Exit: reviewers can explain every crash point and rollback path.
2. **Minimal v2 segment reader/writer prototype:** canonical f32 payload, validated mmap, generation manifest and exact top-K over multiple segments; no ANN. Exit: exact result parity with v1 on deterministic corpus and **no second full f32 matrix** in steady state.
3. **Publication and recovery:** durable flush/switch, concurrent readers, Windows/Unix failure injection, GC retention. Exit: no partial reads, stale references or data loss after acknowledged writes under documented filesystem assumptions.
4. **Storage and kernel measurements:** physical allocated bytes and compaction peak; 100K/1M exact-search warm/cold, measured bandwidth; optional same-machine FAISS Flat exact comparison. Exit: quantified trade-offs, no unsupported speed claims.
5. **Incremental operations:** bounded delta, tombstones, compaction, visibility and backup/restore. Exit: repeated updates do not cause unbounded segment proliferation or break search correctness.
6. **Controlled migration:** dual-read, parity, rollback and versioned ABI negotiation. Exit: v1 data remains usable and consumer compatibility is proven.
7. **Optional acceleration only if needed:** sound exact pruning first; ANN considered separately against actual workload latency/recall constraints.

## Evidence and relationships

- [Corrected source audit and performance model](exact-search-source-audit-2026-11-11.md)
- [Storage-amplification byte model](storage-amplification-and-scale-2026-11-11.md)
- [ADR-0001: v1 format identity](../decisions/0001-preserve-v1-format-identity.md)
- [ADR-0003: accepted architectural direction](../decisions/0003-canonical-packed-segments-and-derived-indexes.md)
- [FAISS official exact index documentation](https://github.com/facebookresearch/faiss/wiki/Faiss-indexes)
- [Lance format architecture](https://lance.org/format/)
- [LSM-tree paper](https://db.cs.berkeley.edu/cs286/papers/lsm-acta1996.pdf)

**No code, format, ABI or publication behavior changes are made by this proposal.**
