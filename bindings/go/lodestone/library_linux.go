//go:build linux && cgo

package lodestone

/*
#cgo LDFLAGS: -ldl

#include <dlfcn.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct LodSearchResult {
    uint64_t id_offset;
    uint32_t id_len;
    uint32_t reserved0;
    float score;
    uint32_t reserved1;
    uint64_t index;
} LodSearchResult;

static void *lod_dlopen(const char *path) {
    dlerror();
    return dlopen(path, RTLD_NOW | RTLD_LOCAL);
}

static void *lod_dlsym(void *handle, const char *name) {
    dlerror();
    return dlsym(handle, name);
}

static const char *lod_dlerror(void) {
    return dlerror();
}

static int lod_dlclose(void *handle) {
    dlerror();
    return dlclose(handle);
}

static uint32_t lod_call_abi_version(void *function) {
    return ((uint32_t (*)(void))function)();
}

static size_t lod_call_last_error(void *function, uint8_t *buffer, size_t capacity) {
    return ((size_t (*)(uint8_t *, size_t))function)(buffer, capacity);
}

static int32_t lod_call_object_exists(
    void *function,
    const uint8_t *store, size_t store_len,
    const uint8_t *model, size_t model_len,
    const uint8_t *source, size_t source_len,
    uint8_t *out_exists
) {
    return ((int32_t (*)(const uint8_t *, size_t, const uint8_t *, size_t, const uint8_t *, size_t, uint8_t *))function)(
        store, store_len, model, model_len, source, source_len, out_exists
    );
}

static int32_t lod_call_ingest_jsonl(
    void *function,
    const uint8_t *store, size_t store_len,
    const uint8_t *model, size_t model_len,
    const uint8_t *input, size_t input_len,
    uint64_t *out_count
) {
    return ((int32_t (*)(const uint8_t *, size_t, const uint8_t *, size_t, const uint8_t *, size_t, uint64_t *))function)(
        store, store_len, model, model_len, input, input_len, out_count
    );
}

static int32_t lod_call_materialize_jsonl(
    void *function,
    const uint8_t *store, size_t store_len,
    const uint8_t *model, size_t model_len,
    const uint8_t *manifest, size_t manifest_len,
    const uint8_t *snapshot, size_t snapshot_len,
    uint8_t *identity, size_t identity_capacity,
    size_t *out_identity_len
) {
    return ((int32_t (*)(const uint8_t *, size_t, const uint8_t *, size_t, const uint8_t *, size_t, const uint8_t *, size_t, uint8_t *, size_t, size_t *))function)(
        store, store_len, model, model_len, manifest, manifest_len, snapshot, snapshot_len,
        identity, identity_capacity, out_identity_len
    );
}

static int32_t lod_call_open_snapshot(
    void *function, const uint8_t *path, size_t path_len, uint64_t *out_handle
) {
    return ((int32_t (*)(const uint8_t *, size_t, uint64_t *))function)(path, path_len, out_handle);
}

static int32_t lod_call_close_snapshot(void *function, uint64_t handle) {
    return ((int32_t (*)(uint64_t))function)(handle);
}

static int32_t lod_call_snapshot_info(
    void *function,
    uint64_t handle,
    uint32_t *out_dimensions,
    uint64_t *out_count,
    uint8_t *model,
    size_t model_capacity,
    size_t *out_model_len
) {
    return ((int32_t (*)(uint64_t, uint32_t *, uint64_t *, uint8_t *, size_t, size_t *))function)(
        handle, out_dimensions, out_count, model, model_capacity, out_model_len
    );
}

static int32_t lod_call_search(
    void *function,
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
    return ((int32_t (*)(uint64_t, const float *, size_t, size_t, LodSearchResult *, size_t, uint8_t *, size_t, size_t *, size_t *))function)(
        handle, query, query_len, top_k, results, results_capacity, ids, ids_capacity,
        out_count, out_ids_len
    );
}
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
	"strings"
	"sync"
	"unsafe"
)

const (
	statusOK             = 0
	statusBufferTooSmall = 2
)

type Library struct {
	path   string
	handle unsafe.Pointer

	lastError    unsafe.Pointer
	objectExists unsafe.Pointer
	ingest       unsafe.Pointer
	materialize  unsafe.Pointer
	open         unsafe.Pointer
	closeHandle  unsafe.Pointer
	info         unsafe.Pointer
	search       unsafe.Pointer

	mu     sync.Mutex
	closed bool
}

type Engine struct {
	library *Library
	handle  uint64

	mu     sync.Mutex
	closed bool
}

func Load(explicit string) (*Library, error) {
	path, err := FindLibrary(explicit)
	if err != nil {
		return nil, err
	}

	pathCString := C.CString(path)
	handle := C.lod_dlopen(pathCString)
	C.free(unsafe.Pointer(pathCString))
	if handle == nil {
		return nil, fmt.Errorf("load Lodestone %s: %s", path, dynamicLoaderError())
	}

	prefix, abi, err := loadABI(handle)
	if err != nil {
		C.lod_dlclose(handle)
		return nil, err
	}
	if version := uint32(C.lod_call_abi_version(abi)); version != 1 {
		C.lod_dlclose(handle)
		return nil, fmt.Errorf("unsupported Lodestone ABI version %d", version)
	}

	library := &Library{path: path, handle: handle}
	procedures := []struct {
		name   string
		target *unsafe.Pointer
	}{
		{"last_error_message", &library.lastError},
		{"object_exists", &library.objectExists},
		{"ingest_jsonl", &library.ingest},
		{"materialize_jsonl", &library.materialize},
		{"open_snapshot", &library.open},
		{"close_snapshot", &library.closeHandle},
		{"snapshot_info", &library.info},
		{"search", &library.search},
	}
	for _, procedure := range procedures {
		found := dynamicSymbol(handle, prefix+procedure.name)
		if found == nil {
			loaderError := dynamicLoaderError()
			C.lod_dlclose(handle)
			return nil, fmt.Errorf("load Lodestone ABI procedure %s%s: %s", prefix, procedure.name, loaderError)
		}
		*procedure.target = found
	}
	return library, nil
}

func loadABI(handle unsafe.Pointer) (string, unsafe.Pointer, error) {
	for _, prefix := range []string{"lod_", "gv_"} {
		if abi := dynamicSymbol(handle, prefix+"abi_version"); abi != nil {
			return prefix, abi, nil
		}
	}
	return "", nil, fmt.Errorf("load Lodestone ABI version procedure")
}

func dynamicSymbol(handle unsafe.Pointer, name string) unsafe.Pointer {
	nameCString := C.CString(name)
	defer C.free(unsafe.Pointer(nameCString))
	return C.lod_dlsym(handle, nameCString)
}

func dynamicLoaderError() string {
	message := C.lod_dlerror()
	if message == nil {
		return "dynamic loader error"
	}
	return C.GoString(message)
}

func (library *Library) Path() string { return library.path }

func (library *Library) Close() error {
	library.mu.Lock()
	defer library.mu.Unlock()
	if library.closed {
		return nil
	}
	library.closed = true
	if C.lod_dlclose(library.handle) != 0 {
		return fmt.Errorf("close Lodestone %s: %s", library.path, dynamicLoaderError())
	}
	return nil
}

func (library *Library) ObjectExists(store, model, source string) (bool, error) {
	storeBytes, modelBytes, sourceBytes := []byte(store), []byte(model), []byte(source)
	var exists C.uint8_t
	status := C.lod_call_object_exists(
		library.objectExists,
		bytePointer(storeBytes), C.size_t(len(storeBytes)),
		bytePointer(modelBytes), C.size_t(len(modelBytes)),
		bytePointer(sourceBytes), C.size_t(len(sourceBytes)),
		&exists,
	)
	runtime.KeepAlive(storeBytes)
	runtime.KeepAlive(modelBytes)
	runtime.KeepAlive(sourceBytes)
	if err := library.status(status); err != nil {
		return false, err
	}
	return exists != 0, nil
}

func (library *Library) IngestJSONL(store, model, input string) (uint64, error) {
	storeBytes, modelBytes, inputBytes := []byte(store), []byte(model), []byte(input)
	var count C.uint64_t
	status := C.lod_call_ingest_jsonl(
		library.ingest,
		bytePointer(storeBytes), C.size_t(len(storeBytes)),
		bytePointer(modelBytes), C.size_t(len(modelBytes)),
		bytePointer(inputBytes), C.size_t(len(inputBytes)),
		&count,
	)
	runtime.KeepAlive(storeBytes)
	runtime.KeepAlive(modelBytes)
	runtime.KeepAlive(inputBytes)
	return uint64(count), library.status(status)
}

func (library *Library) MaterializeJSONL(store, model, manifest, snapshot string) (string, error) {
	storeBytes, modelBytes := []byte(store), []byte(model)
	manifestBytes, snapshotBytes := []byte(manifest), []byte(snapshot)
	identity := make([]byte, 128)
	var identityLength C.size_t
	status := C.lod_call_materialize_jsonl(
		library.materialize,
		bytePointer(storeBytes), C.size_t(len(storeBytes)),
		bytePointer(modelBytes), C.size_t(len(modelBytes)),
		bytePointer(manifestBytes), C.size_t(len(manifestBytes)),
		bytePointer(snapshotBytes), C.size_t(len(snapshotBytes)),
		bytePointer(identity), C.size_t(len(identity)),
		&identityLength,
	)
	runtime.KeepAlive(storeBytes)
	runtime.KeepAlive(modelBytes)
	runtime.KeepAlive(manifestBytes)
	runtime.KeepAlive(snapshotBytes)
	if err := library.status(status); err != nil {
		return "", err
	}
	if identityLength > C.size_t(len(identity)) {
		return "", ErrBufferTooSmall
	}
	return string(identity[:identityLength]), nil
}

func (library *Library) OpenSnapshot(path string) (*Engine, error) {
	pathBytes := []byte(path)
	var handle C.uint64_t
	status := C.lod_call_open_snapshot(
		library.open, bytePointer(pathBytes), C.size_t(len(pathBytes)), &handle,
	)
	runtime.KeepAlive(pathBytes)
	if err := library.status(status); err != nil {
		return nil, err
	}
	return &Engine{library: library, handle: uint64(handle)}, nil
}

func (library *Library) status(status C.int32_t) error {
	if status == statusOK {
		return nil
	}
	if status == statusBufferTooSmall {
		return ErrBufferTooSmall
	}
	message := library.errorMessage()
	if message == "" {
		message = fmt.Sprintf("Lodestone status %d", int32(status))
	}
	return errors.New(message)
}

func (library *Library) errorMessage() string {
	length := C.lod_call_last_error(library.lastError, nil, 0)
	if length == 0 {
		return ""
	}
	buffer := make([]byte, int(length))
	C.lod_call_last_error(library.lastError, bytePointer(buffer), C.size_t(len(buffer)))
	runtime.KeepAlive(buffer)
	return strings.TrimSpace(string(buffer))
}

func (engine *Engine) Info() (Info, error) {
	engine.mu.Lock()
	defer engine.mu.Unlock()
	if engine.closed {
		return Info{}, errors.New("Lodestone snapshot is closed")
	}

	model := make([]byte, 1024)
	var dimensions C.uint32_t
	var count C.uint64_t
	var modelLength C.size_t
	status := C.lod_call_snapshot_info(
		engine.library.info,
		C.uint64_t(engine.handle),
		&dimensions,
		&count,
		bytePointer(model), C.size_t(len(model)),
		&modelLength,
	)
	runtime.KeepAlive(model)
	if err := engine.library.status(status); err != nil {
		return Info{}, err
	}
	if modelLength > C.size_t(len(model)) {
		return Info{}, ErrBufferTooSmall
	}
	return Info{Model: string(model[:modelLength]), Dimensions: int(dimensions), Count: int(count)}, nil
}

func (engine *Engine) Search(query []float32, topK int) ([]Hit, error) {
	engine.mu.Lock()
	defer engine.mu.Unlock()
	if engine.closed {
		return nil, errors.New("Lodestone snapshot is closed")
	}
	if len(query) == 0 || topK <= 0 {
		return nil, errors.New("query and positive topK are required")
	}

	results := make([]C.LodSearchResult, topK)
	ids := make([]byte, max(1024, topK*128))
	for attempts := 0; attempts < 2; attempts++ {
		var count C.size_t
		var idsLength C.size_t
		status := C.lod_call_search(
			engine.library.search,
			C.uint64_t(engine.handle),
			(*C.float)(unsafe.Pointer(unsafe.SliceData(query))), C.size_t(len(query)), C.size_t(topK),
			(*C.LodSearchResult)(unsafe.Pointer(unsafe.SliceData(results))), C.size_t(len(results)),
			bytePointer(ids), C.size_t(len(ids)),
			&count, &idsLength,
		)
		runtime.KeepAlive(query)
		runtime.KeepAlive(results)
		runtime.KeepAlive(ids)
		if status == statusBufferTooSmall {
			if count > C.size_t(len(results)) {
				results = make([]C.LodSearchResult, int(count))
			}
			if idsLength > C.size_t(len(ids)) {
				ids = make([]byte, int(idsLength))
			}
			continue
		}
		if err := engine.library.status(status); err != nil {
			return nil, err
		}
		if count > C.size_t(len(results)) || idsLength > C.size_t(len(ids)) {
			return nil, ErrBufferTooSmall
		}
		hits := make([]Hit, int(count))
		for index, result := range results[:count] {
			start := int(result.id_offset)
			end := start + int(result.id_len)
			if start < 0 || end < start || end > int(idsLength) {
				return nil, fmt.Errorf("Lodestone returned invalid id bounds %d:%d", start, end)
			}
			hits[index] = Hit{ID: string(ids[start:end]), Score: float32(result.score), Index: uint64(result.index)}
		}
		return hits, nil
	}
	return nil, ErrBufferTooSmall
}

func (engine *Engine) Close() error {
	engine.mu.Lock()
	defer engine.mu.Unlock()
	if engine.closed {
		return nil
	}
	status := C.lod_call_close_snapshot(engine.library.closeHandle, C.uint64_t(engine.handle))
	if err := engine.library.status(status); err != nil {
		return err
	}
	engine.closed = true
	return nil
}

func bytePointer(data []byte) *C.uint8_t {
	if len(data) == 0 {
		return nil
	}
	return (*C.uint8_t)(unsafe.Pointer(unsafe.SliceData(data)))
}
