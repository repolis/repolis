package lang

import "testing"

func TestRustNamespace(t *testing.T) {
	l := rustLang{}
	cases := map[string]string{
		// Single-crate layout.
		"src/lib.rs":           "crate",
		"src/main.rs":          "crate",
		"src/layout.rs":        "crate::layout",
		"src/shapes/mod.rs":    "crate::shapes",
		"src/shapes/rect.rs":   "crate::shapes::rect",
		"tests/integration.rs": "crate::integration",

		// Each crate is its own root, so modules must not collide.
		"crates/printer/src/lib.rs":   "printer",
		"crates/printer/src/color.rs": "printer::color",
		"crates/core/flags/defs.rs":   "core::flags::defs",
		"crates/matcher/src/lib.rs":   "matcher",
	}
	for in, want := range cases {
		if got := l.Namespace(in); got != want {
			t.Errorf("Namespace(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestRustImportTargets(t *testing.T) {
	l := rustLang{}
	has := func(list []string, want string) bool {
		for _, v := range list {
			if v == want {
				return true
			}
		}
		return false
	}

	// Relative forms resolve against the importing module.
	got := l.ImportTargets(Import{Path: "super::render"}, "crate::layout::treemap")
	if !has(got, "crate::layout::render") {
		t.Errorf("super:: from crate::layout::treemap gave %v", got)
	}
	got = l.ImportTargets(Import{Path: "self::inner"}, "crate::layout")
	if !has(got, "crate::layout::inner") {
		t.Errorf("self:: gave %v", got)
	}

	// The last segment is often a type, so the parent module must be offered.
	got = l.ImportTargets(Import{Path: "crate::shapes::rect::Rect"}, "crate")
	if !has(got, "crate::shapes::rect") {
		t.Errorf("parent module not offered: %v", got)
	}

	// A sibling is written as the package name, by convention
	// <project>_<crate>, while the directory is just <crate>.
	got = l.ImportTargets(Import{Path: "grep_printer::Standard"}, "core")
	if !has(got, "printer") {
		t.Errorf("workspace sibling grep_printer did not offer %q: %v", "printer", got)
	}
}
