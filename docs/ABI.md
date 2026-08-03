# C ABI and Language Bindings

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines allocation ownership, handle lifetime, concurrency, errors, symbols, and binding compatibility.

## Overview

The C ABI exposes Lodestone through caller-owned buffers and opaque numeric handles. Rust does not retain caller pointers. Open snapshots are stored as `Arc<Snapshot>` values in a synchronized process registry.

## Buffer ownership

- The caller owns all input and output buffers.
- Rust borrows buffers only for the duration of one call.
- Rust does not free caller memory or return heap allocations requiring cross-runtime deallocation.
- Required lengths and error states are reported through the exported contract in `lodestone.h`.

## Handle lifecycle

Opening a snapshot validates it and inserts an `Arc` into the registry under a monotonically allocated numeric handle. Search resolves and clones the `Arc` before scanning. Closing removes the handle from the registry, preventing new operations from resolving it, while any operation holding a clone can finish safely.

An unknown or already closed handle is an error. Handle values are process-local and must not be persisted as storage identity.

## Concurrency

The registry uses a read-write lock. Snapshot data is immutable after validation, so concurrent searches share mappings without mutation. Closing a handle can race safely with an active search because search owns an independent `Arc` clone.

The current registry has no configured handle-count quota. Long-running applications must close unused handles to avoid retaining mappings and address space.

## Error and panic boundary

FFI entry points catch panics before they cross the ABI. Detailed error text is stored per thread and copied into caller-owned output buffers through the exported error contract. Bindings must preserve failure rather than converting invalid data or missing symbols into empty success.

## Symbol compatibility

The Go binding accepts canonical `lod_*` symbols and legacy Grimoire `gv_*` symbols during migration. The compatibility aliases preserve an existing consumer boundary; they do not transfer format or search ownership back to Grimoire.

Removing legacy symbols requires explicit consumer migration evidence and a superseding ADR.

## Related docs

- [Architecture](ARCHITECTURE.md)
- [Operations and recovery](OPERATIONS.md)
- [C header](../crates/ffi/include/lodestone.h)
- [ADR-0002](decisions/0002-temporarily-support-legacy-grimoire-symbols.md)

## Notes

Platform library discovery belongs to each language binding. The C ABI remains the canonical cross-runtime contract.
