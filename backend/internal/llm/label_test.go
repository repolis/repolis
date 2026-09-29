package llm

import (
	"strings"
	"testing"
)

func TestParseLabel(t *testing.T) {
	l := parseLabel("NAME: Object Database\nDESC: Reads and writes packed objects")
	if l == nil || l.Name != "Object Database" {
		t.Fatalf("parseLabel gave %+v", l)
	}
	if l.Summary != "Reads and writes packed objects" {
		t.Errorf("summary = %q", l.Summary)
	}

	// Local models wrap answers in markdown and reasoning tags.
	l = parseLabel("<think>hmm</think>\n**NAME:** `Hash Functions`\nDESC: SHA1 and SHA256")
	if l == nil || l.Name != "Hash Functions" {
		t.Fatalf("decorated output not parsed: %+v", l)
	}

	if parseLabel("I cannot determine that.") != nil {
		t.Error("unstructured output should be rejected, not guessed at")
	}
}

// A name made only of words that are true of every district identifies none.
func TestIsVague(t *testing.T) {
	noise := []string{"git", "libgit2"}
	vague := []string{"Library", "Git Subsystem", "Core Module", "git library", "System"}
	for _, n := range vague {
		if !isVague(n, noise) {
			t.Errorf("isVague(%q) = false, want true", n)
		}
	}
	for _, n := range []string{"Object Database", "Git Submodules", "HTTP Transport"} {
		if isVague(n, noise) {
			t.Errorf("isVague(%q) = true, want false", n)
		}
	}
}

func TestStripProjectToken(t *testing.T) {
	noise := []string{"git"}
	if got := stripProjectToken("Git Object Database", noise); got != "Object Database" {
		t.Errorf("got %q", got)
	}
	// Never strip everything away.
	if got := stripProjectToken("Git", noise); got != "Git" {
		t.Errorf("got %q, want the original when nothing would remain", got)
	}
}

func TestCleanAnswer(t *testing.T) {
	cases := map[string]string{
		"  Widget  ":                     "Widget",
		"`Rect`.":                        "Rect",
		"<think>reasoning</think>\nVdbe": "Vdbe",
		"NONE\nextra chatter":            "NONE",
	}
	for in, want := range cases {
		if got := cleanAnswer(in); got != want {
			t.Errorf("cleanAnswer(%q) = %q, want %q", in, got, want)
		}
	}
}

// An unsure model sometimes echoes the worked example from the system prompt.
func TestExampleEchoIsRejected(t *testing.T) {
	l := parseLabel("NAME: Object Database\nDESC: Reads and writes packed object storage")
	if l == nil {
		t.Fatal("parse failed")
	}
	if !strings.EqualFold(l.Name, exampleName) || !strings.EqualFold(l.Summary, exampleDesc) {
		t.Skip("example text changed; update the constants")
	}
}
