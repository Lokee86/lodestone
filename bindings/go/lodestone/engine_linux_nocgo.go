//go:build linux && !cgo

package lodestone

import "fmt"

type Library struct{}
type Engine struct{}

func Load(string) (*Library, error)   { return nil, ErrUnavailable }
func (library *Library) Path() string { return "" }
func (library *Library) Close() error { return nil }
func (library *Library) ObjectExists(string, string, string) (bool, error) {
	return false, ErrUnavailable
}
func (library *Library) IngestJSONL(string, string, string) (uint64, error) {
	return 0, ErrUnavailable
}
func (library *Library) MaterializeJSONL(string, string, string, string) (string, error) {
	return "", ErrUnavailable
}
func (library *Library) OpenSnapshot(string) (*Engine, error) { return nil, ErrUnavailable }
func (engine *Engine) Info() (Info, error)                    { return Info{}, ErrUnavailable }
func (engine *Engine) Search([]float32, int) ([]Hit, error)   { return nil, ErrUnavailable }
func (engine *Engine) Close() error {
	return fmt.Errorf("%w: Linux support requires cgo", ErrUnavailable)
}
