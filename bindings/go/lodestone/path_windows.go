//go:build windows

package lodestone

import (
	"fmt"
	"os"
	"path/filepath"
	"unsafe"
)

func FindLibrary(explicit string) (string, error) {
	if explicit != "" {
		return requireLibrary(explicit)
	}
	for _, variable := range []string{"LODESTONE_LIBRARY", "GRIMOIRE_VECTOR_ENGINE"} {
		if configured := os.Getenv(variable); configured != "" {
			return requireLibrary(configured)
		}
	}

	executable, _ := os.Executable()
	cwd, _ := os.Getwd()
	for _, candidate := range libraryCandidates(executable, cwd) {
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			return filepath.Abs(candidate)
		}
	}
	return "", fmt.Errorf("%w: set LODESTONE_LIBRARY or build Lodestone", ErrUnavailable)
}

func libraryCandidates(executable, cwd string) []string {
	candidates := make([]string, 0, 64)
	seen := map[string]struct{}{}
	appendCandidate := func(path string) {
		if path == "" {
			return
		}
		path = filepath.Clean(path)
		if _, exists := seen[path]; exists {
			return
		}
		seen[path] = struct{}{}
		candidates = append(candidates, path)
	}
	appendDevelopmentCandidates := func(start string) {
		if start == "" {
			return
		}
		for directory := filepath.Clean(start); ; directory = filepath.Dir(directory) {
			for _, build := range []string{"release", "debug"} {
				appendCandidate(filepath.Join(directory, "target", build, ABIName+".dll"))
				appendCandidate(filepath.Join(directory, "lodestone", "target", build, ABIName+".dll"))
				appendCandidate(filepath.Join(directory, "native", "vector-engine", "target", build, LegacyABIName+".dll"))
			}
			parent := filepath.Dir(directory)
			if parent == directory {
				break
			}
		}
	}

	if executable != "" {
		executableDirectory := filepath.Dir(executable)
		appendCandidate(filepath.Join(executableDirectory, ABIName+".dll"))
		appendCandidate(filepath.Join(executableDirectory, LegacyABIName+".dll"))
		appendDevelopmentCandidates(executableDirectory)
	}
	if cwd != "" {
		appendCandidate(filepath.Join(cwd, ABIName+".dll"))
		appendCandidate(filepath.Join(cwd, LegacyABIName+".dll"))
		appendDevelopmentCandidates(cwd)
	}
	return candidates
}

func requireLibrary(path string) (string, error) {
	absolute, err := filepath.Abs(path)
	if err != nil {
		return "", err
	}
	info, err := os.Stat(absolute)
	if err != nil || info.IsDir() {
		return "", fmt.Errorf("%w: %s", ErrUnavailable, absolute)
	}
	return absolute, nil
}

func bytePointer(data []byte) uintptr {
	if len(data) == 0 {
		return 0
	}
	return uintptr(unsafe.Pointer(unsafe.SliceData(data)))
}
