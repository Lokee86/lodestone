# ADR-0002: Temporarily support legacy Grimoire symbols

## Purpose

This record explains why Lodestone temporarily resolves both canonical `lod_*` and legacy Grimoire `gv_*` ABI symbols.

## Overview

Dual symbol support allows owned consumers to migrate independently without transferring ABI ownership back to Grimoire or requiring one flag-day release.

Status: Accepted
Date: 2026-07-31
Owners: Lodestone C ABI and Go binding
Supersedes: none
Superseded by: none

## Context

Existing consumers were built against Grimoire-prefixed `gv_*` symbols. Lodestone introduces canonical `lod_*` ownership, but requiring every consumer to switch atomically would couple extraction and deployment.

## Decision

The binding accepts both canonical `lod_*` and legacy `gv_*` symbols during migration. The symbols expose the same Lodestone-owned ABI semantics. New integrations use `lod_*`.

## Consequences

### Ownership and dependencies

- Lodestone remains the ABI owner.
- Consumers may migrate independently without a flag day.
- Compatibility code is isolated to the binding and exported symbol boundary.

### State, lifecycle, and operations

- Runtime loading must report which required symbol cannot be resolved.
- Both symbol families must preserve buffer and handle ownership semantics.

### Compatibility and migration

- Legacy support may be removed only after all owned consumers use `lod_*`, release artifacts no longer require `gv_*`, and binding tests prove the canonical path.
- Removal requires a superseding ADR and release note.

## Alternatives considered

- **Immediate symbol break:** rejected because it unnecessarily couples all consumers to one release.
- **Keep only `gv_*`:** rejected because it obscures Lodestone's product ownership indefinitely.

## Verification

- Binding tests against both symbol families.
- Integration checks in Grimoire and Continuity consumers.

## Risks and debt

- Compatibility may become permanent if consumer inventory is not maintained; the decision index must track removal evidence.

## References

- [C ABI and language bindings](../ABI.md)

## Related docs

- [Architecture](../ARCHITECTURE.md)
- [Behavioral-contract matrix](../BEHAVIORAL_CONTRACT_MATRIX.md)

## Notes

Legacy symbols must not become the default for new consumers, and removal requires explicit migration evidence and a superseding decision.
