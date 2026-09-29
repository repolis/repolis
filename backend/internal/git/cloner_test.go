package git

import "testing"

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
