# Documentation Policy

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines canonical documentation ownership, status, coverage, and required update behavior for Lodestone.

## Overview

Documentation is part of persisted-format and ABI compatibility. Object identity, snapshot layout, validation, publication, search, allocation ownership, handle lifetime, recovery, and migration behavior must change with the implementation they describe.

## Canonical ownership

- `ARCHITECTURE.md` owns product boundaries, dependency direction, and state classes.
- `STORAGE.md` owns immutable object identity and write/read behavior.
- `SNAPSHOTS.md` owns materialization, format, validation, publication, and migration.
- `ABI.md` owns cross-runtime allocation, symbols, handles, errors, and concurrency.
- `OPERATIONS.md` owns corruption, interrupted work, recovery, validation, and operational limits.
- `BEHAVIORAL_CONTRACT_MATRIX.md` maps critical invariants to tests and fixtures.
- `limits/` owns current incomplete behavior; `planning/` owns future work; `decisions/` explains durable choices.

## Status rules

Implemented format and compatibility claims require code and fixtures. A snapshot being derived does not make unsafe publication or corruption behavior irrelevant. Planned availability, migration, or format work must not be written as current behavior.

## Change impact

Changes to magic, version, address domain, numeric representation, layout, validation, publication, search ordering, ABI symbols, buffer ownership, handle lifecycle, or binding discovery require review of all affected canonical docs, ADRs, limits, compatibility fixtures, and consumers.

## Related docs

- [Documentation procedure](documentation-procedure.md)
- [Documentation coverage](development/documentation-coverage.md)
- [Architecture](ARCHITECTURE.md)

## Notes

The README is the product introduction. Detailed persisted and cross-runtime contracts belong in the focused documentation owners.
