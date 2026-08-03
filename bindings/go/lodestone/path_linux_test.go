//go:build linux

package lodestone

import (
	"os"
	"path/filepath"
	"slices"
	"testing"
)

func TestLibraryCandidatesSearchSiblingRepositoryBuilds(t *testing.T) {
	root := t.TempDir()
	executable := filepath.Join(root, "grimoire", "bin", "grimoire")
	unrelatedRepository := filepath.Join(t.TempDir(), "repo")

	candidates := libraryCandidates(executable, unrelatedRepository)
	for _, expected := range []string{
		filepath.Join(root, "lodestone", "target", "release", linuxABIName),
		filepath.Join(root, "lodestone", "target", "debug", linuxABIName),
	} {
		if !slices.Contains(candidates, expected) {
			t.Fatalf("expected sibling Lodestone candidate %q in %v", expected, candidates)
		}
	}
}

func TestLibraryCandidatesPreferPackagedSibling(t *testing.T) {
	executable := filepath.Join(t.TempDir(), "bin", "grimoire")
	candidates := libraryCandidates(executable, "")
	expected := filepath.Join(filepath.Dir(executable), linuxABIName)
	if len(candidates) == 0 || candidates[0] != expected {
		t.Fatalf("expected packaged sibling %q first, got %v", expected, candidates)
	}
}

func TestLinuxSystemLibraryCandidatesSearchLDLibraryPathFirst(t *testing.T) {
	first := filepath.Join(t.TempDir(), "first")
	second := filepath.Join(t.TempDir(), "second")
	t.Setenv("LD_LIBRARY_PATH", first+string(os.PathListSeparator)+second)

	candidates := linuxSystemLibraryCandidates()
	expected := []string{
		filepath.Join(first, linuxABIName),
		filepath.Join(first, linuxLegacyABIName),
		filepath.Join(second, linuxABIName),
		filepath.Join(second, linuxLegacyABIName),
	}
	if len(candidates) < len(expected) || !slices.Equal(candidates[:len(expected)], expected) {
		t.Fatalf("expected LD_LIBRARY_PATH candidates %v first, got %v", expected, candidates)
	}
}

func TestFindLibraryHonorsExplicitAndConfiguredPaths(t *testing.T) {
	explicit := filepath.Join(t.TempDir(), linuxABIName)
	configured := filepath.Join(t.TempDir(), linuxABIName)
	for _, path := range []string{explicit, configured} {
		if err := os.WriteFile(path, []byte("test"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	t.Setenv("LODESTONE_LIBRARY", configured)
	t.Setenv("GRIMOIRE_VECTOR_ENGINE", "")

	found, err := FindLibrary(explicit)
	if err != nil {
		t.Fatal(err)
	}
	explicitAbsolute, err := filepath.Abs(explicit)
	if err != nil {
		t.Fatal(err)
	}
	if found != explicitAbsolute {
		t.Fatalf("expected explicit path %q, got %q", explicitAbsolute, found)
	}

	found, err = FindLibrary("")
	if err != nil {
		t.Fatal(err)
	}
	configuredAbsolute, err := filepath.Abs(configured)
	if err != nil {
		t.Fatal(err)
	}
	if found != configuredAbsolute {
		t.Fatalf("expected configured path %q, got %q", configuredAbsolute, found)
	}
}
