package analyzer

import "testing"

func TestTypologyClosedSet(t *testing.T) {
	cases := map[string]string{
		"Object Database":  "data",
		"HTTP Transport":   "network",
		"SHA256 hashing":   "security",
		"test system":      "test", // "test" must win over "system"
		"Widget rendering": "interface",
		"???":              "unknown",
	}
	for in, want := range cases {
		if got := GuessTypology(in); got != want {
			t.Errorf("GuessTypology(%q) = %q, want %q", in, got, want)
		}
	}
}
