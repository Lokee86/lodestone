//go:build linux

package lodestone

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
)

const (
	linuxABIName       = "lib" + ABIName + ".so"
	linuxLegacyABIName = "lib" + LegacyABIName + ".so"
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
	candidates := append(libraryCandidates(executable, cwd), linuxSystemLibraryCandidates()...)
	for _, candidate := range candidates {
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
				appendCandidate(filepath.Join(directory, "target", build, linuxABIName))
				appendCandidate(filepath.Join(directory, "lodestone", "target", build, linuxABIName))
				appendCandidate(filepath.Join(directory, "native", "vector-engine", "target", build, linuxLegacyABIName))
			}
			parent := filepath.Dir(directory)
			if parent == directory {
				break
			}
		}
	}

	if executable != "" {
		executableDirectory := filepath.Dir(executable)
		appendCandidate(filepath.Join(executableDirectory, linuxABIName))
		appendCandidate(filepath.Join(executableDirectory, linuxLegacyABIName))
		appendDevelopmentCandidates(executableDirectory)
	}
	if cwd != "" {
		appendCandidate(filepath.Join(cwd, linuxABIName))
		appendCandidate(filepath.Join(cwd, linuxLegacyABIName))
		appendDevelopmentCandidates(cwd)
	}
	return candidates
}

func linuxSystemLibraryCandidates() []string {
	directories := filepath.SplitList(os.Getenv("LD_LIBRARY_PATH"))
	directories = append(directories, "/usr/local/lib", "/usr/lib", "/lib", "/usr/lib64", "/lib64")
	if triplet := linuxLibraryTriplet(); triplet != "" {
		directories = append(directories, filepath.Join("/usr/lib", triplet), filepath.Join("/lib", triplet))
	}

	candidates := make([]string, 0, len(directories)*2)
	seen := map[string]struct{}{}
	for _, directory := range directories {
		if directory == "" {
			continue
		}
		for _, name := range []string{linuxABIName, linuxLegacyABIName} {
			candidate := filepath.Clean(filepath.Join(directory, name))
			if _, exists := seen[candidate]; exists {
				continue
			}
			seen[candidate] = struct{}{}
			candidates = append(candidates, candidate)
		}
	}
	return candidates
}

func linuxLibraryTriplet() string {
	switch runtime.GOARCH {
	case "amd64":
		return "x86_64-linux-gnu"
	case "arm64":
		return "aarch64-linux-gnu"
	case "386":
		return "i386-linux-gnu"
	case "arm":
		return "arm-linux-gnueabihf"
	default:
		return ""
	}
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
