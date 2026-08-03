# Documentation Procedure

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

This document defines the required workflow for updating Lodestone documentation.

## Overview

Every storage, snapshot, search, ABI, or binding change is reviewed for persisted compatibility, lifecycle, recovery, consumer impact, and protecting evidence.

## Procedure

1. Identify affected object, snapshot, search, ABI, binding, operation, and migration contracts.
2. Update the canonical current-state documents in the same change.
3. Update the maintainer map and coverage when ownership or implementation location moves.
4. Update the behavioral-contract matrix and frozen fixtures when an invariant changes.
5. Add or supersede an ADR for format, identity, publication, compatibility, or cross-runtime ownership decisions.
6. Record incomplete behavior in limits and future work in planning.
7. Update indexes and relative links.
8. Run `cargo test --workspace` and the vendored documentation checker.
9. Verify affected language bindings against the produced library where supported.
10. Report compatibility, migration, documentation, and recovery impact explicitly.

## Removal

Do not remove readers, symbols, or docs solely because new code no longer writes an older format. Removal requires consumer and stored-data evidence plus a superseding decision and migration notice.

## Related docs

- [Documentation policy](documentation-policy.md)
- [Documentation coverage](development/documentation-coverage.md)
- [Architecture decisions](decisions/INDEX.md)

## Notes

Never rewrite an immutable object or snapshot in place to make documentation and implementation appear aligned.
