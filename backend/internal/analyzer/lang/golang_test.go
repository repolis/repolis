package lang

import "testing"

func TestGoNamespace(t *testing.T) {
	l := goLang{}
	cases := map[string]string{
		"main.go":                     "root",
		"store/store.go":              "store",
		"internal/analyzer/lang/c.go": "internal/analyzer/lang",
		"cmd/api/main.go":             "cmd/api",
	}
	for in, want := range cases {
		if got := l.Namespace(in); got != want {
			t.Errorf("Namespace(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestGoImportTargets(t *testing.T) {
	l := goLang{}

	// A module path is offered with successively fewer leading segments, so
	// it can meet a repository-relative namespace wherever the analysis root
	// happens to sit.
	got := l.ImportTargets(Import{Path: "github.com/owner/repo/internal/x"}, "")
	want := []string{
		"github.com/owner/repo/internal/x",
		"owner/repo/internal/x",
		"repo/internal/x",
		"internal/x",
		"x",
	}
	if len(got) != len(want) {
		t.Fatalf("got %v, want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("target %d = %q, want %q", i, got[i], want[i])
		}
	}

	// The standard library is not in the repository. Offering "os" would let
	// it suffix-match a local internal/os and invent edges into it.
	for _, std := range []string{"os", "fmt", "net/http", "encoding/json"} {
		if n := l.ImportTargets(Import{Path: std}, ""); len(n) != 0 {
			t.Errorf("stdlib %q produced targets %v", std, n)
		}
	}
}
