# Documentation Coverage

Parent index: [Lodestone Development Documentation](INDEX.md)

## Purpose

This document maps implementation responsibilities to canonical documentation owners.

## Overview

Coverage follows persisted and cross-runtime responsibility rather than package count.

| Implementation area | Responsibility | Canonical documentation |
| --- | --- | --- |
| `crates/core/src/object.rs`, `object_format.rs`, `format.rs` | Object identity, encoding, validation, conflict behavior, durable publication | [Storage](../STORAGE.md), [Operations](../OPERATIONS.md) |
| `crates/core/src/materialize.rs`, `materialize_format.rs` | Deterministic snapshot construction and target publication | [Snapshots](../SNAPSHOTS.md), [Operations](../OPERATIONS.md) |
| `crates/core/src/snapshot.rs` and format modules | Snapshot validation and mmap lifetime | [Snapshots](../SNAPSHOTS.md), [Architecture](../ARCHITECTURE.md) |
| `crates/core/src/search.rs` | Exact nearest-neighbour scan and deterministic top-K ordering | [Architecture](../ARCHITECTURE.md), [Behavioral contracts](../BEHAVIORAL_CONTRACT_MATRIX.md) |
| `crates/ffi/`, `crates/ffi/include/lodestone.h` | C ABI symbols, caller buffers, errors, panics, handles, concurrency | [ABI](../ABI.md), [Operations](../OPERATIONS.md) |
| `bindings/go/lodestone/` | Go library discovery, symbol loading, and Go-facing lifecycle | [ABI](../ABI.md), binding README |
| `crates/cli/` | Diagnostic ingest, build, inspect, and search composition | Repository README, [Operations](../OPERATIONS.md) |
| `.github/workflows/`, crate/binding tests | Release and invariant evidence | [Behavioral-contract matrix](../BEHAVIORAL_CONTRACT_MATRIX.md) |

## Related docs

- [Maintainer map](../MAINTAINER_MAP.md)
- [Documentation policy](../documentation-policy.md)
- [Current limitations](../limits/current-limitations.md)

## Notes

A binding may own platform discovery, but it must not redefine object identity, snapshot validity, result ordering, or ABI allocation ownership.
