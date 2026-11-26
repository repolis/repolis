package main

import (
	"os"
	"path/filepath"
	"testing"
)

const unreachableRepo = "https://127.0.0.1:1/owner/repo"

func TestResolveCommitFallsBackToLastCachedCommit(t *testing.T) {
	t.Chdir(t.TempDir())
	if err := os.MkdirAll(filepath.Join("data", "cities"), 0o755); err != nil {
		t.Fatal(err)
	}

	if _, err := resolveCommit(unreachableRepo); err == nil {
		t.Fatal("an unreachable repository with nothing cached resolved")
	}

	rememberCommit(unreachableRepo, "abc123def456")
	commit, err := resolveCommit(unreachableRepo)
	if err != nil || commit != "abc123def456" {
		t.Errorf("resolveCommit = %q, %v; want the last cached commit", commit, err)
	}
}
