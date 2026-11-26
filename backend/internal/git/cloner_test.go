package git

import (
	"os/exec"
	"strings"
	"testing"
)

// NormalizeRepoURL is a security boundary: git treats some strings as
// transports that run shell commands, and a leading "-" as a flag.
func TestNormalizeRepoURLRejectsInjection(t *testing.T) {
	bad := []string{
		"",
		"ext::sh -c 'id > /tmp/pwned'",
		"--upload-pack=touch /tmp/pwned",
		"-u root",
		"file:///etc/passwd",
		"http://github.com/a/b",
		"ssh://git@github.com/a/b",
		"git@github.com:a/b.git",
		"https://evil.com/a/b",
		"https://user:pass@github.com/a/b",
		"https://github.com/a/b/../../c",
		"https://github.com/only-one-segment",
		"https://github.com/a/b/c/d",
		"https://github.com/a/b;rm -rf /",
	}
	for _, in := range bad {
		if got, err := NormalizeRepoURL(in); err == nil {
			t.Errorf("NormalizeRepoURL(%q) accepted it as %q, want rejection", in, got)
		}
	}
}

func TestNormalizeRepoURLCanonicalises(t *testing.T) {
	cases := map[string]string{
		"https://github.com/rgamble/libcsv":      "https://github.com/rgamble/libcsv",
		"https://github.com/rgamble/libcsv.git":  "https://github.com/rgamble/libcsv",
		"https://github.com/rgamble/libcsv/":     "https://github.com/rgamble/libcsv",
		"  https://GitHub.com/tsoding/nothing  ": "https://github.com/tsoding/nothing",
		"https://gitlab.com/o/r":                 "https://gitlab.com/o/r",
	}
	for in, want := range cases {
		got, err := NormalizeRepoURL(in)
		if err != nil {
			t.Errorf("NormalizeRepoURL(%q) errored: %v", in, err)
			continue
		}
		if got != want {
			t.Errorf("NormalizeRepoURL(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestIsCommitHashRejectsAnythingButAFullHash(t *testing.T) {
	bad := []string{"", "HEAD", "main", "abc123", "--upload-pack=x", strings.Repeat("A", 40), strings.Repeat("a", 41)}
	for _, in := range bad {
		if IsCommitHash(in) {
			t.Errorf("IsCommitHash(%q) = true", in)
		}
	}
	if !IsCommitHash(strings.Repeat("a1", 20)) {
		t.Error("a full lowercase hash was rejected")
	}
}

func TestCheckoutCommitMovesToAnOlderCommit(t *testing.T) {
	dir := t.TempDir()
	git := func(args ...string) string {
		cmd := exec.Command("git", append([]string{"-c", "user.name=t", "-c", "user.email=t@t"}, args...)...)
		cmd.Dir = dir
		out, err := cmd.CombinedOutput()
		if err != nil {
			t.Fatalf("git %v: %v: %s", args, err, out)
		}
		return strings.TrimSpace(string(out))
	}
	git("init", "--quiet")
	git("commit", "--quiet", "--allow-empty", "-m", "first")
	first := git("rev-parse", "HEAD")
	git("commit", "--quiet", "--allow-empty", "-m", "second")

	if err := CheckoutCommit(dir, first); err != nil {
		t.Fatal(err)
	}
	if head := git("rev-parse", "HEAD"); head != first {
		t.Errorf("HEAD = %s, want %s", head, first)
	}
}
