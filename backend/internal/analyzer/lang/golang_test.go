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

	// Successively fewer leading segments, so the path can meet a
	// repo-relative namespace wherever the analysis root sits.
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

	// Offering "os" would suffix-match a local internal/os and invent edges.
	for _, std := range []string{"os", "fmt", "net/http", "encoding/json"} {
		if n := l.ImportTargets(Import{Path: std}, ""); len(n) != 0 {
			t.Errorf("stdlib %q produced targets %v", std, n)
		}
	}
}
