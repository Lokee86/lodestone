# Lodestone Behavioral-Contract Matrix

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This matrix maps critical storage, snapshot, search, and ABI invariants to protecting evidence.

## Overview

The matrix exposes coverage and gaps; tests and compatibility fixtures remain the actual enforcement.

| Invariant | Canonical owner | Implementation boundary | Protecting evidence | Failure meaning |
| --- | --- | --- | --- | --- |
| One model/source address cannot contain different vector bytes | [Storage](STORAGE.md) | `ObjectStore::put` | `reuses_identical_object_and_rejects_conflict` | Immutable identity violated |
| New objects are written through a synchronized temporary file before rename | Storage | object writer | Core object tests and file inspection | Partial object could appear at an address |
| Snapshot record order and identity are deterministic | [Snapshots](SNAPSHOTS.md) | `validate_records`, layout, writer | `snapshot_is_deterministic_and_sorted` | Rebuilds cannot be compared or reproduced |
| Invalid snapshot bytes are rejected before search | Snapshots | `Snapshot::open` and format validation | Snapshot corruption and validation tests | Untrusted offsets or vectors reached search |
| Search uses one validated immutable snapshot | [Architecture](ARCHITECTURE.md) | `search.rs`, mmap snapshot | Core search tests | Results may mix generations or invalid data |
| Closing a handle cannot invalidate an active search | [ABI](ABI.md) | registry `Arc` clone before scan | FFI concurrency tests or integration evidence | Use-after-close across ABI |
| Caller memory is never retained or freed by Rust | ABI | exported C functions | Header contract and FFI tests | Cross-runtime allocation violation |
| Panics do not cross the ABI | ABI | FFI boundary wrappers | FFI failure tests | Undefined behavior across language boundary |
| v1 objects and snapshots retain their format identity | [ADR-0001](decisions/0001-preserve-v1-format-identity.md) | format constants and decoders | Existing-store fixtures and compatibility tests | Existing persisted data becomes unreadable or misread |
| Legacy `gv_*` symbols remain aliases during migration | [ADR-0002](decisions/0002-temporarily-support-legacy-grimoire-symbols.md) | Go loader and exported ABI | Binding tests for both symbol families | Existing consumers fail before cutover |

## Known gaps

- Existing-target snapshot replacement does not yet have a dedicated interrupted-publication test matrix across Windows and Unix semantics.
- Handle-count exhaustion and application leak behavior are documented but not bounded by the engine.
- Cross-version object and snapshot fixtures should be expanded before a v2 format is introduced.

## Related docs

- [Storage](STORAGE.md)
- [Snapshots](SNAPSHOTS.md)
- [ABI](ABI.md)
- [Operations](OPERATIONS.md)

## Notes

Add invariant-to-test links when test names or fixture paths stabilize; do not claim coverage from documentation alone.
