# ADR-0001: Preserve v1 format identity

## Purpose

This record explains why Lodestone preserves the existing version-1 object and snapshot identities after extraction from Grimoire.

## Overview

Extraction changes product ownership, not persisted meaning. Existing immutable objects and snapshots remain readable under their original explicit v1 identity while future incompatible formats receive new versions.

Status: Accepted
Date: 2026-07-31
Owners: Lodestone core storage and snapshot formats
Supersedes: none
Superseded by: none

## Context

Lodestone was extracted from an existing Grimoire vector-storage implementation. Existing immutable objects and snapshots already use a version-1 address domain and snapshot magic. Changing those bytes during extraction would strand existing stores or require an unnecessary rewrite migration.

## Decision

Lodestone preserves the existing v1 object-address domain, object encoding identity, snapshot magic, and aligned `float32` representation. Future incompatible formats receive a new explicit version and coexist through compatibility readers or migration tooling; v1 data is never silently reinterpreted.

## Consequences

### Ownership and dependencies

- Lodestone core owns the format constants and validation.
- Consumers depend on explicit version compatibility, not internal Rust layout.
- Applications remain responsible for rebuilding snapshots from objects and manifests when moving to a new version.

### State, lifecycle, and operations

- Existing stores remain reusable after extraction.
- Corrupt v1 data is rejected, not upgraded in place.
- Compatibility fixtures become release evidence.

### Compatibility and migration

- A v2 format requires a new ADR, new address or magic identity, fixtures, and a bounded migration path.
- Removing v1 readers requires explicit consumer and stored-data evidence.

## Alternatives considered

- **Create a new format immediately:** rejected because extraction alone did not justify invalidating existing data.
- **Silently translate files on open:** rejected because it would mix read, migration, and publication authority.

## Verification

- Core object and snapshot tests.
- Existing Grimoire v1 store and snapshot compatibility evidence.

## Risks and debt

- Dedicated frozen cross-version fixtures need expansion before v2 work begins.

## References

- [Storage](../STORAGE.md)
- [Snapshots](../SNAPSHOTS.md)

## Related docs

- [Operations and recovery](../OPERATIONS.md)
- [Behavioral-contract matrix](../BEHAVIORAL_CONTRACT_MATRIX.md)

## Notes

A future v2 must use a new explicit identity and migration path rather than silently changing what v1 bytes mean.
