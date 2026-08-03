# Lodestone Maintainer Map

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document routes changes to their implementation, documentation, and verification owners.

## Overview

Storage and compatibility changes are cross-cutting even when the code edit is small. Use this map before changing format bytes, ABI symbols, or lifecycle behavior.

| Change area | Implementation owner | Canonical docs | Verification |
| --- | --- | --- | --- |
| Object addressing, encoding, conflict handling | `crates/core/src/object.rs`, `object_format.rs`, `format.rs` | [Storage](STORAGE.md), decisions | Core object tests and compatibility fixtures |
| Snapshot layout and materialization | `materialize.rs`, `materialize_format.rs` | [Snapshots](SNAPSHOTS.md) | Determinism, layout, failure, and migration tests |
| Snapshot validation and mmap | `snapshot.rs`, format modules | [Snapshots](SNAPSHOTS.md), [Operations](OPERATIONS.md) | Corruption and boundary fixtures |
| Exact search and ranking order | `search.rs` | [Architecture](ARCHITECTURE.md) | Search tests and benchmark evidence |
| C ABI symbols, buffers, errors, handles | `crates/ffi/`, `lodestone.h` | [ABI](ABI.md) | FFI tests and consumer integration |
| Go loading and platform discovery | `bindings/go/lodestone/` | [Go binding README](../bindings/go/README.md), [ABI](ABI.md) | Windows/Linux binding tests |
| Diagnostic CLI | `crates/cli/` | README and [Operations](OPERATIONS.md) | CLI smoke tests |
| Format or compatibility decisions | Cross-cutting owner | [Decision index](decisions/INDEX.md) | ADR, fixtures, and affected consumer evidence |
| Critical invariants | Core and binding owners | [Behavioral-contract matrix](BEHAVIORAL_CONTRACT_MATRIX.md) | Focused tests and CI |

## Related docs

- [Architecture](ARCHITECTURE.md)
- [Behavioral-contract matrix](BEHAVIORAL_CONTRACT_MATRIX.md)
- [Architecture decisions](decisions/README.md)

## Notes

Do not change v1 magic, address domains, or exported ownership semantics as an implementation cleanup.
