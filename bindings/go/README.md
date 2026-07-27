# Lodestone Go binding

This module provides a small Go wrapper around Lodestone's stable C ABI.

```go
library, err := lodestone.Load("")
engine, err := library.OpenSnapshot("index.lod")
hits, err := engine.Search(query, 20)
```

On Windows the loader checks `LODESTONE_LIBRARY`, a library beside the executable, and sibling Lodestone development builds. `GRIMOIRE_VECTOR_ENGINE` and the legacy `grimoire_vector_ffi` ABI remain accepted during migration.

The non-Windows package currently returns `ErrUnavailable`; native Unix loaders are planned separately.
