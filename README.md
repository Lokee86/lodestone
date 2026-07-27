# Lodestone

Lodestone is a fast, embedded, file-native vector database for applications that do not want SQL or a separate database service.

It owns content-addressed immutable vector objects, compact aligned search snapshots, memory-mapped validation, and exact nearest-neighbour search. Embedding generation is deliberately outside Lodestone: applications may use any local model or embedding endpoint and supply the resulting vectors.

## Workspace

```text
crates/core   immutable objects, snapshot format, mmap validation, parallel search
crates/ffi    narrow C ABI with caller-owned buffers and opaque numeric handles
crates/cli    diagnostic ingest, build, inspect, and search commands
bindings/go   shared Go loader used by Grimoire and Continuity Memory
```

## Build

```bash
cargo build -p lodestone-ffi --release
cargo build -p lodestone-cli --release
```

On Windows the library is emitted as `target/release/lodestone_ffi.dll`; the command-line executable is `target/release/lodestone.exe`.

## Storage model

Each immutable vector object is addressed by BLAKE3 over:

```text
format identity + embedding identity + source-content hash
```

The object stores the embedding identity, source identity, dimensions, and little-endian `float32` vector. Reusing the same model/source address with different vector bytes is rejected.

A materialized snapshot contains:

- a fixed versioned header;
- embedding identity;
- sorted record-ID entries and object hashes;
- a compact UTF-8 record-ID table; and
- one 64-byte-aligned contiguous `float32` vector matrix.

Snapshots are validated before use and then memory-mapped. Search performs exact dot products, uses a bounded per-worker top-K heap, scans small snapshots serially, and partitions larger snapshots through Rayon.

Lodestone does not interpret documents, generate embeddings, or own hybrid ranking. Those remain application concerns.

## CLI

```bash
lodestone ingest --store ./store --model model-v1 --input vectors.jsonl
lodestone build --store ./store --model model-v1 --manifest records.jsonl --snapshot index.lod
lodestone inspect --snapshot index.lod
lodestone search --snapshot index.lod --query 0.1,0.2,0.3 --top-k 10
```

The ingest JSONL format is:

```json
{"source":"content-digest","vector":[0.1,0.2,0.3]}
```

The build manifest JSONL format is:

```json
{"id":"application-record-id","source":"content-digest"}
```

## ABI ownership

The C ABI avoids cross-runtime allocation ownership:

- callers own every input and output buffer;
- Rust borrows those buffers only for one call;
- Rust never retains a caller pointer;
- Rust-owned snapshots live behind numeric handles in a synchronized registry;
- search clones an `Arc` before scanning, so closing a handle cannot invalidate active work;
- panics are caught before crossing the ABI.

See `crates/ffi/include/lodestone.h` for the exported contract. The Go module under `bindings/go` wraps the same ABI and accepts both Lodestone's `lod_*` symbols and the legacy Grimoire `gv_*` symbols during migration.

## Compatibility

The initial Lodestone release preserves the existing version-1 Grimoire object-address domain and snapshot magic. This allows existing Grimoire stores and snapshots to remain reusable after extraction. Those bytes are part of the immutable v1 format identity; future formats will use an explicit new version rather than silently rewriting existing data.

## Verification

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p lodestone-ffi --release
```

Version 1 stores aligned `float32` vectors. Quantized encodings are intentionally deferred until retrieval-quality and end-to-end latency benchmarks justify them.

Licensed under Apache 2.0.
