#ifndef LODESTONE_H
#define LODESTONE_H

#include <stddef.h>
#include <stdint.h>

#ifdef _WIN32
#define LOD_API __declspec(dllimport)
#else
#define LOD_API
#endif

typedef struct LodSearchResult {
    uint64_t id_offset;
    uint32_t id_len;
    uint32_t reserved0;
    float score;
    uint32_t reserved1;
    uint64_t index;
} LodSearchResult;

LOD_API uint32_t lod_abi_version(void);
LOD_API size_t lod_last_error_message(uint8_t *buffer, size_t capacity);
LOD_API int32_t lod_object_exists(const uint8_t*, size_t, const uint8_t*, size_t, const uint8_t*, size_t, uint8_t*);
LOD_API int32_t lod_ingest_jsonl(const uint8_t*, size_t, const uint8_t*, size_t, const uint8_t*, size_t, uint64_t*);
LOD_API int32_t lod_materialize_jsonl(const uint8_t*, size_t, const uint8_t*, size_t, const uint8_t*, size_t, const uint8_t*, size_t, uint8_t*, size_t, size_t*);
LOD_API int32_t lod_open_snapshot(const uint8_t*, size_t, uint64_t*);
LOD_API int32_t lod_close_snapshot(uint64_t);
LOD_API int32_t lod_snapshot_info(uint64_t, uint32_t*, uint64_t*, uint8_t*, size_t, size_t*);
LOD_API int32_t lod_search(uint64_t, const float*, size_t, size_t, LodSearchResult*, size_t, uint8_t*, size_t, size_t*, size_t*);

#endif
