package git

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

func run(t *testing.T, dir string, args ...string) {
	t.Helper()
	cmd := exec.Command("git", args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(),
		"GIT_AUTHOR_NAME=Alice", "GIT_AUTHOR_EMAIL=a@example.com",
		"GIT_COMMITTER_NAME=Alice", "GIT_COMMITTER_EMAIL=a@example.com",
	)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, out)
	}
}

// One repo-wide pass must produce the same per-file churn the old three
// subprocesses-per-file version did.
func TestExtractHistory(t *testing.T) {
	dir := t.TempDir()
	run(t, dir, "init", "-q", "-b", "main")
	run(t, dir, "config", "user.name", "Alice")
	run(t, dir, "config", "user.email", "a@example.com")

	write := func(name, body string) {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(body), 0o644); err != nil {
			t.Fatal(err)
		}
	}

	write("a.c", "int a;\n")
	run(t, dir, "add", ".")
	run(t, dir, "commit", "-qm", "one")

	write("a.c", "int a; int b;\n")
	write("b.c", "int b;\n")
	run(t, dir, "add", ".")
	run(t, dir, "commit", "-qm", "two")

	write("a.c", "int a; int b; int c;\n")
	run(t, dir, "add", ".")
	run(t, dir, "commit", "-qm", "three")

	h, err := ExtractHistory(dir)
	if err != nil {
		t.Fatalf("ExtractHistory: %v", err)
	}
	if h["a.c"] == nil || h["a.c"].Churn != 3 {
		t.Errorf("a.c churn = %v, want 3", h["a.c"])
	}
	if h["b.c"] == nil || h["b.c"].Churn != 1 {
		t.Errorf("b.c churn = %v, want 1", h["b.c"])
	}
	if h["a.c"].PrimaryAuthor != "Alice" {
		t.Errorf("primary author = %q", h["a.c"].PrimaryAuthor)
	}
	if h["a.c"].LastModified.IsZero() {
		t.Error("last modified not recorded")
	}
	if h["a.c"].LastModified.Before(h["b.c"].LastModified) {
		t.Error("a.c should be at least as recent as b.c")
	}
}
