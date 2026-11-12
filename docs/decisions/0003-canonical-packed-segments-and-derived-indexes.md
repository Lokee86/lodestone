# ADR-0003: Converge on canonical packed segments and derived search indexes

## Purpose

Record the selected **target architectural direction** for reducing redundant vector storage while supporting efficient incremental and approximate retrieval. This ADR accepts the direction, **not** a specific binary layout, ANN algorithm, migration mechanism, or implementation.

Status: Accepted (direction only; implementation pending)
Date: 2026-11-11
Owners: Lodestone core storage, publication and search
Supersedes: none
Superseded by: none

## Context

Lodestone v1 preserves vectors in content-addressed immutable object files and duplicates full `f32` payloads in packed materialized snapshots. This gives independently validated immutable writes and contiguous mmap-friendly exact scanning, but incurs approximately two full payload copies when every active row has a unique object and one snapshot is retained. Small object files also incur filesystem overhead. See the [storage-amplification investigation](../planning/storage-amplification-and-scale-2026-11-11.md) for the actual byte model and its assumptions. Neither total disk superiority over competitors nor an ANN winner has been measured.

Continuously updated, larger collections also need incremental index maintenance and sublinear-style candidate retrieval without abandoning the embedded, file-native design.

## Decision: target architecture

1. **One canonical full-precision vector payload** per retained logical vector/identity where feasible. Favor immutable packed **content-addressed segments** as authoritative vector storage, rather than individual object payloads plus another entire vector matrix in every search snapshot. Retain independently checkable content identities even when many vectors share a segment file.
2. **Search indexes are derived acceleration structures**, not alternate authorities: exact, partition/IVF, HNSW, or compressed candidate indexes should reference stable vector identities or segment/offset locators. Never require an additional full-size `f32` payload merely to rebuild an index.
3. **Immutable published generations** with manifest-based references: introduce a bounded mutable or append/delta intake path, flush to immutable segments, and consolidate through bounded compaction. Queries merge eligible segments and delta state; tombstones and version visibility must be deterministic.
4. **Safe cross-generation reference resolution:** segment identity, checksums, offset bounds and mapping lifetime are validated; reference indirection survives compaction and publication. Index files are disposable and rebuildable, but vector authority must be recoverable after interrupted writes.
5. **Filter-aware retrieval** with application-owned scope semantics. The engine may expose partitions/filter interfaces to avoid cross-scope ANN top-K contamination; it must not absorb Reliquary memory/tenant policy.
6. **Evidence-led indexing choice:** keep exact search as baseline and compare at least HNSW and IVF-Flat (then quantization/PQ where warranted), optimizing total **allocated bytes per searchable vector** subject to latency, recall, update cost and peak-disk requirements—not simply the snapshot file size.

## Non-goals and not-yet-decided details

- No immediate rewrite or deletion of v1 object/snapshot data, and no mutation of frozen v1 format identity.
- No asserted segment size, hash-address scheme, stable-ID mapping, manifest binary format, delta write protocol, compaction algorithm, ANN choice or index defaults. Each requires an explicit reviewed design and compatibility evidence.
- No requirement to retain physical single-file storage for the complete database if doing so harms safe incremental publication; target a compact number of immutable, mmap-friendly files and a small independently published manifest. Evaluate a single-file container separately.
- No claim that offsets alone remain stable across compaction: use generation-scoped locators and/or identity-to-location resolution, with explicit retention and garbage-collection rules.
- No assumption that vector payload duplication can always be zero: compaction, generation coexistence, backup, copy-on-write and replication may transiently retain data. Measure steady-state and peak amplification separately.

## Required acceptance gates before implementation

1. Build a reproducible v1 physical-footprint, ingestion/refresh, search and crash-recovery baseline, with 10K/100K/1M and a later 10M scale gate; include the object-store/snapshot double-count and filesystem allocation.
2. Freeze compatibility fixtures for v1 objects, snapshots, address domains and cross-runtime behavior. Any authoritative segment representation requires explicit new version/identity and a migration/dual-read plan.
3. Design and test content identity versus segment identity, append/flush and durability boundaries, generation manifests, mapping lifetime, tombstone semantics, compaction atomicity and interrupted publication.
4. Prototype shared-payload exact search first; demonstrate correct equality/ordering, no additional complete payload copy in steady state, and acceptable random access/page-cache cost.
5. Evaluate ANN prototypes against filtered recall, p50/p95/p99 latency, index bytes, RSS, build/incremental insert costs, corruption rejection and rebuild behavior.
6. Preserve v1 readers and a rollback path until migration evidence supports retirement; deploy behind explicit version/capability negotiation.

## Alternatives considered

- **Keep v1 object + snapshot duplication and only add HNSW:** simplest extension but compounds storage amplification; not the preferred long-term direction.
- **Keep individual object files and search them through pointers:** minimizes copies but can cause poor locality, many file handles and filesystem metadata; remains a benchmarkable intermediate option.
- **Make HNSW itself authoritative:** rejected; search topology is rebuildable and should not define canonical vector identity or durability.
- **Store only a packed snapshot with no separate content identities:** rejected because it loses object-level integrity/identity and generation recovery properties.

## Consequences and follow-up

This direction makes vector identity distinct from physical location, raises the importance of generation-scoped reference/GC management and requires new persisted contracts. It may remove most steady-state vector duplication **if measurements validate the design**, while leaving authoritative immutable bytes independent of chosen search indexes.

Follow-up work belongs in [the roadmap](../planning/roadmap.md) and [storage investigation](../planning/storage-amplification-and-scale-2026-11-11.md). Current [storage](../STORAGE.md), [snapshots](../SNAPSHOTS.md) and [architecture](../ARCHITECTURE.md) documents continue to describe **implemented v1**, not this proposed future system.
