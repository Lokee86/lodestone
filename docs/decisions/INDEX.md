# Lodestone Architecture Decisions

Parent index: [Lodestone Documentation](../INDEX.md)

## Purpose

This index records consequential storage, format, ABI, publication, and compatibility decisions.

## Overview

ADRs explain why a durable choice was made. Current storage, snapshot, ABI, and operations documents remain authoritative for implemented behavior.

## Decisions

| ADR | Status | Decision |
| --- | --- | --- |
| [0001](0001-preserve-v1-format-identity.md) | Accepted | Preserve the extracted Grimoire v1 object-address and snapshot identities |
| [0002](0002-temporarily-support-legacy-grimoire-symbols.md) | Accepted | Support legacy `gv_*` ABI symbols during consumer migration |
| [0003](0003-canonical-packed-segments-and-derived-indexes.md) | Accepted (direction; pending design) | Target canonical immutable packed vector segments and rebuildable search indexes |

## Related docs

- [Architecture](../ARCHITECTURE.md)
- [Storage](../STORAGE.md)
- [Snapshots](../SNAPSHOTS.md)
- [ABI](../ABI.md)
- [Compatibility link](README.md)

## Notes

ADRs explain why. Current contracts remain canonical in the focused documents.
