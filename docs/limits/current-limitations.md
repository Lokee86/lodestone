# Current Limitations

Parent index: [Lodestone Limits](INDEX.md)

## Purpose

This document records current limitations that materially affect availability, compatibility, recovery, or resource use.

## Overview

Lodestone has strong immutable storage and validation, but snapshot replacement, compatibility fixtures, and process resource policy still have incomplete surfaces.

## Limitations

### Existing-target v1 snapshot replacement

The builder writes a complete temporary file, but on platforms where rename cannot replace an existing target it removes the target before retrying. A path opener can observe a missing-target window, and failure after removal can leave the target unavailable.

**Removal condition:** cross-platform generation publication provides atomic pointer switching or equivalent uninterrupted validated availability with failure tests.

### Handle limits

The ABI registry has no configured handle-count or mapped-byte quota. Applications that do not close old generations can retain mappings and exhaust process resources.

**Removal condition:** the engine or binding exposes measured optional limits and actionable diagnostics without invalidating active searches.

### Version fixtures

Current v1 tests protect behavior, but frozen object, snapshot, ABI, and binding fixtures are not yet comprehensive enough for a v2 migration.

**Removal condition:** a cross-version fixture suite covers accepted, rejected, migrated, and legacy-consumer paths before v2 ships.

### Automated repair

Corrupt objects and snapshots are rejected and rebuilt from authoritative application state, but no diagnostic repair command inventories temp files, validates generations, and produces a bounded recovery plan.

**Removal condition:** a repair/inspect surface remains read-first, never patches immutable bytes in place, and records every promoted or removed artifact.

### Publication availability contract

V1 consumers that require uninterrupted availability can use generation-named snapshots. The packed store has [Rust publication and reclamation contracts](../PACKED_STORE.md) with verified process interruption recovery. Reclamation protects the current generation and those with open readers.

**Removal condition:** supported consumers can use the packed-store publication contract.

## Related docs

- [Snapshot format and publication](../SNAPSHOTS.md)
- [Operations and recovery](../OPERATIONS.md)
- [Roadmap](../planning/roadmap.md)

## Notes

Lodestone being embedded does not transfer these lifecycle decisions to an undocumented application convention.
