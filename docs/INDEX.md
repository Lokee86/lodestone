# Lodestone Documentation

## Purpose

This index routes maintainers to Lodestone's canonical architecture, storage, snapshot, ABI, operations, verification, and decision documentation.

## Overview

Lodestone is an embedded file-native vector database. Immutable objects preserve vector identity; materialized snapshots provide compact validated mmap search state; the C ABI and Go bindings expose that engine without transferring allocation ownership across runtimes.

## Direct files

- [Architecture](ARCHITECTURE.md)
- [Immutable object storage](STORAGE.md)
- [Snapshot format and publication](SNAPSHOTS.md)
- [Packed vector rows](PACKED_VECTORS.md)
- [C ABI and language bindings](ABI.md)
- [Operations and recovery](OPERATIONS.md)
- [Maintainer map](MAINTAINER_MAP.md)
- [Behavioral-contract matrix](BEHAVIORAL_CONTRACT_MATRIX.md)
- [Architecture decisions](decisions/INDEX.md)
- [Documentation policy](documentation-policy.md)
- [Documentation procedure](documentation-procedure.md)
- [Development coverage](development/INDEX.md)
- [Current limitations](limits/INDEX.md)
- [Planning](planning/INDEX.md)

## Related docs

- [Repository README](../README.md)
- [Go binding README](../bindings/go/README.md)
- [C header](../crates/ffi/include/lodestone.h)

## Notes

The README remains the product introduction. These focused documents own the durable technical contracts.
