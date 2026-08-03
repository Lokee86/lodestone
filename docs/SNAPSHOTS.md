# Snapshot Format and Publication

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines snapshot contents, validation, publication behavior, compatibility, and interrupted-build recovery.

## Overview

A materialized snapshot is a derived immutable search artifact containing a versioned header, embedding identity, sorted record entries, object hashes, a UTF-8 record-ID table, and one aligned contiguous `float32` vector matrix.

## Materialization

Materialization requires a non-empty model and record set. Record IDs and source identities must be non-empty; record IDs must be unique. Records are sorted by ID before layout and writing so the same inputs produce the same snapshot identity and order.

Every referenced object must exist and match the selected embedding identity and dimensions. Materialization fails rather than omitting invalid records.

## Validation and opening

A snapshot is validated before use. Validation covers format identity, version, bounds, offsets, dimensions, record count, string regions, matrix alignment, and other structural invariants owned by the format modules. Only a fully valid file is memory-mapped and exposed to search.

Invalid or truncated snapshots are rejected. Already-open mappings remain valid even if the path is later replaced or removed by another process.

## Publication behavior

The current implementation writes a complete unique temporary file, then renames it to the requested target. When a target already exists and direct replacement rename fails, Lodestone removes the existing target and retries the rename.

Consequences:

- readers using an already-open mmap are unaffected;
- a reader opening by path can observe a narrow missing-target window during replacement on platforms that cannot replace by rename;
- if replacement fails after removing the old target, materialization returns an error and the temporary file may require cleanup;
- the target is reopened and validated after publication before success is returned.

True cross-platform atomic replace is not currently guaranteed for an existing target. Applications requiring uninterrupted path availability should publish generation-named snapshots and switch an application-owned pointer only after `Snapshot::open` succeeds.

## Recovery

After interrupted or failed materialization:

1. Treat a missing or invalid target as unavailable; do not search it.
2. Identify uniquely named `.tmp-<pid>-<sequence>` files adjacent to the target.
3. Validate no active builder owns the file.
4. Remove stale temporary files or validate and manually promote one only through a controlled repair procedure.
5. Rebuild from the immutable object store and authoritative record manifest.

## Format migration

A new snapshot layout or numeric encoding requires a new explicit version and compatibility fixtures. Existing v1 snapshots remain readable while supported; they are not rewritten in place. Applications migrate by building a new-version snapshot from immutable objects and switching only after validation.

## Related docs

- [Architecture](ARCHITECTURE.md)
- [Immutable object storage](STORAGE.md)
- [Operations and recovery](OPERATIONS.md)
- [ADR-0001](decisions/0001-preserve-v1-format-identity.md)

## Notes

Snapshot identity is derived state. The application remains responsible for the record manifest that determines which immutable objects belong in a generation.
