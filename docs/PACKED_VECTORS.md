# Packed Vector Rows

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines Lodestone's filesystem-independent packed-vector substrate. It is intentionally separate from the v1 object and snapshot formats.

## Overview

`lodestone-packed` provides only a validated dense fixed-row byte matrix and scalar schema. Persistence, embedding identity, metrics, quantization semantics, and domain row identity remain caller-owned.

## Representation

The dependency-light `lodestone-packed` crate owns this representation. `VectorSchema` describes one dense matrix with a non-zero dimension count and one scalar representation. `PackedVectors` stores rows contiguously with no per-row framing:

```text
[row 0][row 1][row 2] ...
```

Row width is exactly `dimensions * scalar_width`. The current scalar tags cover signed and unsigned 8/16/32/64-bit integers plus IEEE/bfloat 16-bit, 32-bit, and 64-bit floating representations.

The packed substrate treats row bytes as representation data. It does not define quantization scale/zero-point semantics, embedding identity, similarity metric, record identity, persistence, publication, or application ownership. Those belong to the caller or to a higher-level Lodestone format.

## Validation

- dimensions must be non-zero;
- row-byte calculation must not overflow;
- a packed byte region must contain only complete rows;
- explicit row construction rejects rows with the wrong byte length;
- ordinal lookup returns exactly one fixed-width row.

## Compatibility

This API does not alter or reinterpret Lodestone v1 objects or snapshots. The existing v1 `f32` snapshot format remains unchanged. A future persisted format using `PackedVectors` requires its own explicit format identity.

## Related docs

- [Architecture](ARCHITECTURE.md)
- [Immutable object storage](STORAGE.md)
- [Snapshot format and publication](SNAPSHOTS.md)

## Notes

The type exists so hosts such as Continuity can reuse Lodestone's dense row layout and schema validation while retaining ownership of their own durable container format.
