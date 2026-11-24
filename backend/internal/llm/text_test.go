package llm

import "testing"

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
