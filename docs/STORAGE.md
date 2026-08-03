# Immutable Object Storage

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines immutable object identity, write behavior, validation, conflict handling, and recovery.

## Overview

Each vector object is addressed by BLAKE3 over the v1 format identity, embedding identity, and source-content identity. Vector bytes are stored inside the object but are not allowed to vary for an existing address.

## Write contract

`ObjectStore::put`:

1. validates non-empty identities and a valid finite vector;
2. computes the deterministic object address;
3. returns the existing object when its decoded vector is identical;
4. rejects an existing address containing different vector bytes;
5. writes a new encoded object through a uniquely named temporary file;
6. synchronizes file contents before publication;
7. renames the temporary file to the immutable object path;
8. handles a concurrent identical publisher by validating the winning object.

Writers never update an object in place.

## Read and corruption behavior

Reads recompute the expected address and decode the entire object against the requested model and source identity. Missing files return a missing-object error. Invalid format, identity, dimensions, or vector bytes return an error; callers must not use partial data.

A corrupt immutable object is not repaired in place. Recovery requires regenerating the correct embedding from the authoritative application source and writing it under the same identity only when the resulting bytes match the intended v1 object.

## Temporary files

Failed writes attempt to remove their temporary files. A host crash can leave dot-prefixed temporary files in an object shard. They are not valid objects and are never addressed by readers. Operators may remove them after confirming no active Lodestone writer owns them.

## Compatibility

The v1 address domain is immutable. Changes to encoding, identity fields, numeric representation, or hash domain require a new explicit format version. Silent reinterpretation would break content addressing and existing stores.

## Related docs

- [Architecture](ARCHITECTURE.md)
- [Snapshot format and publication](SNAPSHOTS.md)
- [Operations and recovery](OPERATIONS.md)
- [ADR-0001](decisions/0001-preserve-v1-format-identity.md)

## Notes

Application source identity must itself be stable and collision-resistant enough for the application's correctness requirements; Lodestone does not define how source-content digests are produced.
