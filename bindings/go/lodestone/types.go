package lodestone

import "errors"

const (
	ABIName       = "lodestone_ffi"
	LegacyABIName = "grimoire_vector_ffi"
)

var (
	ErrUnavailable    = errors.New("Lodestone is unavailable")
	ErrBufferTooSmall = errors.New("Lodestone output buffer is too small")
)

type Info struct {
	Model      string `json:"model"`
	Dimensions int    `json:"dimensions"`
	Count      int    `json:"count"`
}

type Hit struct {
	ID    string  `json:"id"`
	Score float32 `json:"score"`
	Index uint64  `json:"index"`
}
