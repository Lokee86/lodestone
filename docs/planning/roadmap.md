# Lodestone Roadmap

Parent index: [Lodestone Planning](INDEX.md)

## Purpose

This document prioritizes work that closes current availability, recovery, resource, and compatibility gaps.

## Overview

Near-term work should harden publication and version evidence before introducing a new storage format or broader product surface.

## Priorities

1. Define and test a generation-based snapshot publication and switch contract across Windows and Unix semantics.
2. Add frozen v1 object, snapshot, ABI, and binding fixtures before any v2 design begins.
3. Add read-first diagnostic tooling for stale temp files, invalid snapshots, missing objects, and retained handle pressure.
4. Define optional handle/mapping resource limits and observability for long-running consumers.
5. Complete cross-platform Go binding verification and release packaging.
6. Pursue the [accepted architectural direction](../decisions/0003-canonical-packed-segments-and-derived-indexes.md) through the [v2 canonical packed-segment design proposal](v2-canonical-segments-design-2026-11-11.md): one authoritative full-precision vector payload, stable semantic identities, generation-scoped locators, validated manifests, incremental compaction and versioned migration. This is **proposed v2**, not implemented behavior.
7. Use the [corrected exact-search source audit](exact-search-source-audit-2026-11-11.md) to keep exact NN the default and avoid unverified cross-hardware performance claims. Establish a small, reproducible v1/v2 physical-footprint, exact top-K, recovery and measured-bandwidth baseline; optional same-machine FAISS Flat comparison only where it resolves a design question.
8. Consider safe exact pruning, ANN indexes or numeric encodings only when measured workloads justify them, behind explicit format identities and compatibility plans. See [Storage amplification and scale investigation](storage-amplification-and-scale-2026-11-11.md).

## Related docs

- [Current limitations](../limits/current-limitations.md)
- [Snapshots](../SNAPSHOTS.md)
- [ABI](../ABI.md)
- [Behavioral-contract matrix](../BEHAVIORAL_CONTRACT_MATRIX.md)

## Notes

Embedding generation, hybrid ranking, and application memory policy remain outside Lodestone's roadmap unless the product boundary is explicitly reconsidered.
