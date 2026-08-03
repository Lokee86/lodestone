//go:build linux && cgo

package lodestone

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

const testLibrarySource = `
#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef struct LodSearchResult {
    uint64_t id_offset;
    uint32_t id_len;
    uint32_t reserved0;
    float score;
    uint32_t reserved1;
    uint64_t index;
} LodSearchResult;

uint32_t PREFIXabi_version(void) { return 1; }

size_t PREFIXlast_error_message(uint8_t *buffer, size_t capacity) {
    const char *message = "test error";
    size_t length = strlen(message);
    if (buffer != NULL && capacity > 0) {
        size_t count = capacity < length ? capacity : length;
        memcpy(buffer, message, count);
    }
    return length;
}

int32_t PREFIXobject_exists(
    const uint8_t *store, size_t store_len,
    const uint8_t *model, size_t model_len,
    const uint8_t *source, size_t source_len,
    uint8_t *out_exists
) {
    *out_exists = 1;
    return 0;
}

int32_t PREFIXingest_jsonl(
    const uint8_t *store, size_t store_len,
    const uint8_t *model, size_t model_len,
    const uint8_t *input, size_t input_len,
    uint64_t *out_count
) {
    *out_count = 2;
    return 0;
}

int32_t PREFIXmaterialize_jsonl(
    const uint8_t *store, size_t store_len,
    const uint8_t *model, size_t model_len,
    const uint8_t *manifest, size_t manifest_len,
    const uint8_t *snapshot, size_t snapshot_len,
    uint8_t *identity, size_t identity_capacity,
    size_t *out_identity_len
) {
    const char *value = "snapshot-id";
    size_t length = strlen(value);
    *out_identity_len = length;
    if (identity_capacity < length) {
        return 2;
    }
    memcpy(identity, value, length);
    return 0;
}

int32_t PREFIXopen_snapshot(const uint8_t *path, size_t path_len, uint64_t *out_handle) {
    *out_handle = 42;
    return 0;
}

int32_t PREFIXclose_snapshot(uint64_t handle) {
    return handle == 42 ? 0 : 3;
}

int32_t PREFIXsnapshot_info(
    uint64_t handle,
    uint32_t *out_dimensions,
    uint64_t *out_count,
    uint8_t *model,
    size_t model_capacity,
    size_t *out_model_len
) {
    const char *value = "test-model";
    size_t length = strlen(value);
    *out_dimensions = 2;
    *out_count = 2;
    *out_model_len = length;
    if (model_capacity < length) {
        return 2;
    }
    memcpy(model, value, length);
    return 0;
}

int32_t PREFIXsearch(
    uint64_t handle,
    const float *query,
    size_t query_len,
    size_t top_k,
    LodSearchResult *results,
    size_t results_capacity,
    uint8_t *ids,
    size_t ids_capacity,
    size_t *out_count,
    size_t *out_ids_len
) {
    const char *id = "alpha";
    *out_count = 1;
    *out_ids_len = 5;
    if (results_capacity < 1 || ids_capacity < 5) {
        return 2;
    }
    results[0].id_offset = 0;
    results[0].id_len = 5;
    results[0].score = 0.75f;
    results[0].index = 7;
    memcpy(ids, id, 5);
    return 0;
}
`

func TestLoadAndCallLinuxSharedLibrary(t *testing.T) {
	compiler, err := exec.LookPath("cc")
	if err != nil {
		t.Skip("C compiler is unavailable")
	}

	for _, prefix := range []string{"lod_", "gv_"} {
		t.Run(strings.TrimSuffix(prefix, "_"), func(t *testing.T) {
			root := t.TempDir()
			source := filepath.Join(root, "lodestone.c")
			libraryPath := filepath.Join(root, linuxABIName)
			if err := os.WriteFile(source, []byte(strings.ReplaceAll(testLibrarySource, "PREFIX", prefix)), 0o644); err != nil {
				t.Fatal(err)
			}
			command := exec.Command(compiler, "-shared", "-fPIC", "-std=c11", "-o", libraryPath, source)
			if output, err := command.CombinedOutput(); err != nil {
				t.Fatalf("compile test library: %v\n%s", err, output)
			}

			library, err := Load(libraryPath)
			if err != nil {
				t.Fatal(err)
			}
			if library.Path() != libraryPath {
				t.Fatalf("expected library path %q, got %q", libraryPath, library.Path())
			}

			exists, err := library.ObjectExists("store", "test-model", "source")
			if err != nil || !exists {
				t.Fatalf("exists=%v err=%v", exists, err)
			}
			count, err := library.IngestJSONL("store", "test-model", "input.jsonl")
			if err != nil || count != 2 {
				t.Fatalf("count=%d err=%v", count, err)
			}
			identity, err := library.MaterializeJSONL("store", "test-model", "manifest.jsonl", "snapshot.lod")
			if err != nil || identity != "snapshot-id" {
				t.Fatalf("identity=%q err=%v", identity, err)
			}

			engine, err := library.OpenSnapshot("snapshot.lod")
			if err != nil {
				t.Fatal(err)
			}
			info, err := engine.Info()
			if err != nil || info != (Info{Model: "test-model", Dimensions: 2, Count: 2}) {
				t.Fatalf("info=%+v err=%v", info, err)
			}
			hits, err := engine.Search([]float32{1, 0}, 1)
			if err != nil {
				t.Fatal(err)
			}
			expected := Hit{ID: "alpha", Score: 0.75, Index: 7}
			if len(hits) != 1 || hits[0] != expected {
				t.Fatalf("expected hit %+v, got %+v", expected, hits)
			}
			if _, err := engine.Search(nil, 1); err == nil {
				t.Fatal("empty query unexpectedly succeeded")
			}
			if err := engine.Close(); err != nil {
				t.Fatal(err)
			}
			if _, err := engine.Info(); err == nil {
				t.Fatal("info after close unexpectedly succeeded")
			}
			if err := library.Close(); err != nil {
				t.Fatal(err)
			}
			if err := library.Close(); err != nil {
				t.Fatalf("second close: %v", err)
			}
		})
	}
}

func TestLoadRejectsUnsupportedLinuxABI(t *testing.T) {
	compiler, err := exec.LookPath("cc")
	if err != nil {
		t.Skip("C compiler is unavailable")
	}
	root := t.TempDir()
	source := filepath.Join(root, "unsupported.c")
	libraryPath := filepath.Join(root, linuxABIName)
	if err := os.WriteFile(source, []byte("#include <stdint.h>\nuint32_t lod_abi_version(void) { return 2; }\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	command := exec.Command(compiler, "-shared", "-fPIC", "-o", libraryPath, source)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("compile test library: %v\n%s", err, output)
	}

	_, err = Load(libraryPath)
	if err == nil || !strings.Contains(err.Error(), "unsupported Lodestone ABI version 2") {
		t.Fatalf("expected unsupported ABI error, got %v", err)
	}
	if errors.Is(err, ErrUnavailable) {
		t.Fatalf("unsupported ABI error should not be unavailable: %v", err)
	}
}
