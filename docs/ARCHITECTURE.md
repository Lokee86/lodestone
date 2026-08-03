# Lodestone Architecture

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines Lodestone's responsibility owners, dependency direction, state model, and non-ownership boundaries.

## Overview

Lodestone separates immutable vector storage from materialized search state. Applications write content-addressed objects, build a versioned snapshot from record references, open and validate that snapshot, and perform exact nearest-neighbour search against a memory map.

## Ownership

| Area | Owner | Responsibilities |
| --- | --- | --- |
| Immutable objects and format identity | `crates/core/src/object*.rs` | Addressing, validation, encoding, durable write, conflict rejection, read |
| Snapshot materialization and format | `crates/core/src/materialize*.rs` | Deterministic record ordering, matrix construction, temporary output, publication |
| Snapshot validation and mmap lifecycle | `crates/core/src/snapshot.rs` and format modules | Header, offsets, identity, dimensions, bounds, mapping, metadata |
| Exact search | `crates/core/src/search.rs` | Query validation, serial/parallel scan, bounded top-K, deterministic result ordering |
| C ABI | `crates/ffi/` and `lodestone.h` | Buffer ownership, error boundary, opaque handles, panic containment |
| Go binding | `bindings/go/lodestone/` | Platform library discovery, symbol loading, Go-facing types and lifecycle |
| Diagnostic CLI | `crates/cli/` | Ingest, build, inspect, and search composition |

## Dependency direction

```text
CLI ---------> core
C ABI -------> core
Go binding --> C ABI dynamic library
applications -> Go binding or C ABI

core -/-> application embedding or ranking policy
core -/-> bindings
```

The core crate owns storage and search semantics. Bindings adapt those semantics but cannot redefine object identity, snapshot validity, ranking, or lifetime guarantees.

## State model

- **Authoritative immutable state:** object files addressed by format identity, embedding identity, and source identity.
- **Derived immutable state:** materialized snapshots that reference object hashes and contain a search-ready vector matrix.
- **Process-local state:** validated memory maps and opaque snapshot handles.
- **Application-owned state:** embeddings before ingestion, record selection, hybrid ranking, document metadata, and business policy.

A snapshot can be rebuilt from immutable objects plus the application manifest. It is not the authority for source content or embedding generation.

## Lifecycle

1. Validate model/source identity and vector shape.
2. Write a new object to a unique temporary file, sync it, and rename it to its immutable address.
3. Validate and sort record references for snapshot materialization.
4. Write the complete snapshot to a temporary path.
5. Publish the target path, then reopen and validate the published snapshot.
6. Open snapshots through the core or ABI, retaining a memory map behind a process-local handle.
7. Search clones a stable snapshot reference before scanning.
8. Closing a handle removes future lookup access but does not invalidate an active cloned reference.

## Non-ownership

Lodestone does not own:

- embedding model execution or endpoint selection;
- document parsing, chunking, memory policy, or metadata authority;
- lexical retrieval or hybrid reranking;
- application-level record deletion policy beyond building a new snapshot;
- network service discovery or a daemon lifecycle.

## Related docs

- [Immutable object storage](STORAGE.md)
- [Snapshot format and publication](SNAPSHOTS.md)
- [C ABI and language bindings](ABI.md)
- [Operations and recovery](OPERATIONS.md)

## Notes

Lodestone is intentionally embedded. Adding a service wrapper does not move storage or format ownership out of the core engine.
