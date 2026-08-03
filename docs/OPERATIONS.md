# Operations and Recovery

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines operational checks, failure handling, corruption response, publication recovery, compatibility checks, and resource limits.

## Overview

Lodestone has no daemon. Operational health is established by successful object validation, snapshot opening, ABI loading, and focused verification in the embedding application or diagnostic CLI.

## Failure handling

| Failure | Required response |
| --- | --- |
| Missing immutable object | Fail materialization or read; regenerate from authoritative source and embedding identity |
| Conflicting bytes at an object address | Reject the write; investigate source/model identity misuse |
| Corrupt object | Quarantine or remove only after evidence; regenerate rather than patching in place |
| Invalid or truncated snapshot | Refuse to open; rebuild from objects and manifest |
| Interrupted snapshot build | Remove stale temp files after confirming no active writer; rebuild |
| Replacement publication failure | Treat target availability as uncertain; validate target and temp state before retry |
| Unsupported format version | Refuse to interpret; use a compatible engine or explicit migration |
| Missing ABI symbol or library | Fail binding initialization with platform and path evidence |
| Unknown/closed handle | Reject operation; open a new validated snapshot |
| Handle exhaustion or mapping pressure | Close unused handles and bound application snapshot generations |

## Validation workflow

Before releasing or switching a snapshot generation:

1. Build from an authoritative record manifest.
2. Open the produced file through `Snapshot::open` or the target binding.
3. Inspect model identity, dimensions, record count, and snapshot identity.
4. Run at least one representative search through the same ABI or binding used in production.
5. Switch the application-owned generation pointer only after validation.

## Observability

The diagnostic CLI exposes ingest, build, inspect, and search results. The ABI exposes per-thread last-error text. Applications should log snapshot path or generation, embedding identity, dimensions, record count, load failure, and search failure without logging full vectors by default.

## Backups and repair

Immutable object stores are suitable for incremental file backup. A snapshot can be regenerated and is lower-value backup state than the objects plus authoritative application manifest. Repair must never edit object or snapshot bytes in place; create a new valid artifact and replace application references after validation.

## Related docs

- [Immutable object storage](STORAGE.md)
- [Snapshot format and publication](SNAPSHOTS.md)
- [C ABI and language bindings](ABI.md)
- [Behavioral-contract matrix](BEHAVIORAL_CONTRACT_MATRIX.md)

## Notes

Applications with strict availability requirements should use generation-named snapshot files because the current replacement path does not guarantee uninterrupted path availability on every platform.
