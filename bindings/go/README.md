# Lodestone Go binding

This module provides a small Go wrapper around Lodestone's stable C ABI.

```go
library, err := lodestone.Load("")
engine, err := library.OpenSnapshot("index.lod")
hits, err := engine.Search(query, 20)
```

On Windows and Linux the loader checks `LODESTONE_LIBRARY`, a library beside the executable, and sibling Lodestone development builds. Linux also searches `LD_LIBRARY_PATH` and standard system library directories for `liblodestone_ffi.so`. `GRIMOIRE_VECTOR_ENGINE` and the legacy `grimoire_vector_ffi` ABI remain accepted during migration.

Linux support uses cgo and `dlopen`; a C toolchain and `CGO_ENABLED=1` are required. Other non-Windows platforms continue to return `ErrUnavailable`.
