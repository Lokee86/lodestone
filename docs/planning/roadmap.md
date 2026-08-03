# Lodestone Roadmap

Parent index: [Lodestone Planning](INDEX.md)

## Purpose

This document prioritizes work that closes current availability, recovery, resource, and compatibility gaps.

## Overview

Near-term work should harden publication and version evidence before introducing a new storage format or broader product surface.

## Priorities

1. Define and test a generation-based snapshot publication and switch contract across Windows and Unix semantics.
2. Add frozen v1 object, snapshot, ABI, and binding fixtures before any v2 design begins.
3. Add read-first diagnostic tooling for stale temp files, invalid snapshots, missing objects, and retained handle pressure.
4. Define optional handle/mapping resource limits and observability for long-running consumers.
5. Complete cross-platform Go binding verification and release packaging.
6. Introduce new approximate indexes or numeric encodings only behind explicit format identities and measured compatibility plans.

## Related docs

- [Current limitations](../limits/current-limitations.md)
- [Snapshots](../SNAPSHOTS.md)
- [ABI](../ABI.md)
- [Behavioral-contract matrix](../BEHAVIORAL_CONTRACT_MATRIX.md)

## Notes

Embedding generation, hybrid ranking, and application memory policy remain outside Lodestone's roadmap unless the product boundary is explicitly reconsidered.
