package analyzer

import (
	"os"
	"path/filepath"
	"testing"
)

// A workspace member owns a licence and manifest exactly like a vendored
// library; treating that as vendoring deleted three of ripgrep's own crates.
func TestWorkspaceMemberIsNotVendored(t *testing.T) {
	dir := t.TempDir()
	crate := filepath.Join(dir, "printer")
	if err := os.MkdirAll(filepath.Join(crate, "src"), 0o755); err != nil {
		t.Fatal(err)
	}
	for _, f := range []string{"Cargo.toml", "LICENSE", "README.md"} {
		if err := os.WriteFile(filepath.Join(crate, f), []byte("x"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if isVendoredDir(crate) {
		t.Error("a workspace crate with a licence and a manifest was treated as vendored")
	}

	// A licence beside a pre-built binary still is.
	vendored := filepath.Join(dir, "libfoo")
	if err := os.MkdirAll(vendored, 0o755); err != nil {
		t.Fatal(err)
	}
	for _, f := range []string{"LICENSE", "libfoo.a"} {
		if err := os.WriteFile(filepath.Join(vendored, f), []byte("x"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if !isVendoredDir(vendored) {
		t.Error("a licence next to a pre-compiled binary should be treated as vendored")
	}
}
