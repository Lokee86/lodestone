# Packed store publication and recovery

Parent index: [Lodestone Documentation](INDEX.md)

## Purpose

Define the Rust packed store's publication, read, compaction, and reclamation contracts for a single embedding profile.

## Overview

A packed store is a dedicated, existing directory on a local filesystem. The profile names one model and fixes the vector dimension. Each model/source address has one retained full-precision row. A logical record binds its own ID to that address, and other records may share the row. Search scans mapped rows with the exact dot-product ranker. The view owns its mappings and returns ties in record ID order.

Use PackedStore to create or open the profile. PackedStore::append publishes one batch. For a larger atomic append, use begin_append, stage each bounded batch, and commit once. A stage error makes the update ineligible to commit. Open a new view with open_current to read the latest generation. Existing views search independently of the store or writer.

~~~rust
use lodestone_core::{PackedRecord, PackedStore};

let store = PackedStore::create(existing_directory, "embedding-profile", 2)?;
let mut update = store.begin_append()?;
update.stage(&[PackedRecord {
    id: "record-1",
    source: "document-1",
    vector: &[0.5, -0.25],
}])?;
let generation = update.commit()?;
let view = store.open_current()?;
assert_eq!(view.number(), generation);
assert_eq!(view.search(&[1.0, 0.0], 1)?[0].id, "record-1");
~~~

## Owner-published matrices

Applications with an existing publication boundary can keep their packed matrix in that store and use the same bounded exact ranking used by the directory store. Call search_packed_cosine with a borrowed PackedVectors matrix and a finite nonzero F32 query. It scans all rows using f64 cosine similarity and returns row ordinals and scores. Tied scores follow ordinal order. Zero rows have undefined similarity and are skipped, while a non-finite row fails the search. The caller retains ownership of the matrix, its logical IDs, and its publication. The packed directory generation continues to use its exact dot-product contract.

## Commit and recovery

1. An exclusive lock on the profile serializes writers across processes. A new view holds the shared profile lock while selecting and validating its generation and acquiring a shared lock on the manifest. It releases the profile lock before searching. The manifest lock stays with the view until its segment mappings close.
2. Write each segment to a unique staging file, flush it, and give it a fresh immutable final name. Validate the segment checksum, layout, finite vectors, and record bindings before referencing it.
3. Build a checksummed manifest with the full set of old and new segment references. Flush it in staging and publish its final generation name. Unix uses a same-directory hard link and syncs the directory. Windows uses a same-volume MoveFileExW call with MOVEFILE_WRITE_THROUGH and create-only naming. Commit acknowledges only after the naming durability step succeeds. [Linux fsync contract](https://man7.org/linux/man-pages/man2/fsync.2.html) · [Windows move contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
4. Reopen selects the highest finalized generation number. Staging files and segments without a reference in that manifest are unreachable. An interruption during naming can leave either complete generation visible. A finalized manifest or referenced segment that fails validation results in an error.
5. After an ambiguous commit, reopen and retry the same record IDs, sources, and bytes. Exact repeats reuse the retained rows and issue a fresh manifest with its own durability barrier. Conflicting record bindings or model/source bytes are rejected.

An empty profile is generation zero. Published file names are create-only, which keeps previous readers independent of subsequent publication.

## Compaction

PackedStore::compact takes a maximum number of distinct source rows per replacement segment. The writer lock covers selection of the current generation, repacking, and publication. Source rows are grouped with their record bindings, including aliases from other segments. The store validates every replacement segment and checks all source identities, exact vector bytes, and record bindings against the starting generation before it can publish.

The replacement manifest refers only to the new segments and uses the same durable commit as an append. The returned generation number acknowledges that commit. An empty store or a target that cannot reduce the active segment count returns None and publishes nothing.

Compaction retains the earlier manifests and segment files. Existing views own their original mappings and can continue searching during and after the switch. Call PackedStore::reclaim to retire earlier generations when their readers have released them.

## Reclamation

PackedStore::reclaim validates the current generation under the profile writer lock. It probes earlier manifests with a nonblocking exclusive lock and protects all segments referenced by the current or any pinned manifest. Already-open views can search throughout the pass. The OS releases a view's pin when its last handle closes or its process exits. [Windows file locks](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex) · [Unix file locks](https://man7.org/linux/man-pages/man2/flock.2.html)

Cleanup gives each unpinned older manifest a retired name before sweeping regular managed files. Unix syncs the directory before deleting any segments. Windows uses the same write-through move as publication to retire the name. Segments referenced by any protected generation remain available, including shared segments in otherwise retired generations. Stale staging files and retired manifest names are removed as part of a completed pass.

An interrupted pass can be retried. Each pass finds its roots again, and files already removed require no bookkeeping. The selected generation stays readable and keeps its number. The returned counts describe retirements and segment removals performed by that pass. An initialized empty store yields zero removals.

## Versioned files

| File | Contract |
| --- | --- |
| PACKED | LDSPRF02 followed by a UTF-8 JSON model and dimension, with a trailing BLAKE3 digest of the magic and JSON |
| generation-N.ldg | LDSMAN02 followed by UTF-8 JSON and the same digest envelope, with N formatted as 20 decimal digits |
| .retired-N.ldg | A retired generation manifest with the same encoding, eligible for removal during reclamation |
| segment-D-T.lds | LDSSEG02 segment, where D is its full-file BLAKE3 digest and T is a unique 16-character alphanumeric file token |

A manifest contains its number, the previous manifest digest if any, the profile digest, and an ordered list of segment digests and file tokens. The previous digest records lineage and can outlive predecessor files. Reopening needs the current manifest and every segment it lists.

Each segment starts with a 64-byte header. It stores dimensions as little-endian u32 at byte 8 and the source count, binding count, and metadata length as little-endian u64 at bytes 16, 24, and 32. Reserved bytes are zero. Metadata contains the source strings in row order and then the bindings. Every string uses a little-endian u32 byte length followed by UTF-8. A binding stores its record ID and the 32-byte model/source address. Zero padding aligns the matrix to 64 bytes. The matrix stores little-endian float32 values with no per-row padding.

## Verification

The append and compaction tests start a separate writer process, pause it at each staging, flush, naming, and acknowledgement boundary, and kill it. They search an existing mapped reader while the writer is paused and after recovery. A fresh open and retry verify the selected generation and its exact top-K results. Other tests check shared sources, retained file bytes, concurrent append and compaction, and corruption of finalized files. [Publication tests](../crates/core/src/packed_store/tests.rs)

Reclamation tests keep multiple views of an old generation, kill a separate reader process to release its pin, and kill the reclaimer during retirement, segment removal, and metadata cleanup. A fresh open and cleanup retry verify exact results and that only reachable segments survive. [Reclamation tests](../crates/core/src/packed_store/tests.rs)

The interruption tests passed at 11 boundaries each for append and compaction, and at five for reclamation, on Windows 11 with NTFS and on Alpine Linux 3.24, kernel 6.18.60, with ext4 in a local guest. The reader-process termination test also passed on both platforms. These tests exercise process termination. The durability ordering relies on the local filesystem's documented flush contracts.

## Related docs

- [V1 snapshot publication](SNAPSHOTS.md)
- [Storage direction](decisions/0003-canonical-packed-segments-and-derived-indexes.md)
- [Packed-segment benchmark](development/packed-segment-benchmark.md)
- [Operational recovery](OPERATIONS.md)

## Notes

The durability contract assumes an established local directory and storage that honors flushes. Apply all updates through the store API and keep segment files immutable while they can be mapped. The packed-store publication API is Rust-only at this stage.
