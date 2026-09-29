package analyzer

import (
	"testing"
)

const rustFixture = "testdata/rustfixture"

func rustDraft(t *testing.T) *Draft {
	t.Helper()
	raw, err := ExtractRepository(rustFixture)
	if err != nil {
		t.Fatalf("ExtractRepository: %v", err)
	}
	return BuildDraft(raw, nil, nil)
}

func TestRustNamespaces(t *testing.T) {
	raw, err := ExtractRepository(rustFixture)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]string{
		"src/lib.rs":           "crate",
		"src/util.rs":          "crate::util",
		"src/shapes/mod.rs":    "crate::shapes",
		"src/shapes/rect.rs":   "crate::shapes::rect",
		"src/shapes/circle.rs": "crate::shapes::circle",
	}
	for _, f := range raw.Files {
		if w, ok := want[f.Path]; ok && f.Namespace != w {
			t.Errorf("%s namespace = %q, want %q", f.Path, f.Namespace, w)
		}
		if f.Language != "Rust" {
			t.Errorf("%s language = %q", f.Path, f.Language)
		}
	}
}

// An `impl` block states the owner. Nothing should be guessed, and a free
// function must stay free however suggestive its parameters are.
func TestRustAssociationIsSyntacticOnly(t *testing.T) {
	raw, err := ExtractRepository(rustFixture)
	if err != nil {
		t.Fatal(err)
	}
	st := BuildSymbolTable(raw)

	get := func(name string) *Assoc {
		for _, a := range st.Assocs {
			if a.Fn.Name == name {
				return a
			}
		}
		t.Fatalf("function %s not extracted", name)
		return nil
	}

	// Methods inside `impl Rect`.
	for _, m := range []string{"area", "new"} {
		found := false
		for _, a := range st.Assocs {
			if a.Fn.Name == m && a.Fn.Receiver == "Rect" {
				found = true
				if a.Source != SourceSyntax || a.Confidence != ConfHigh {
					t.Errorf("Rect::%s source=%s confidence=%s, want syntax/high", m, a.Source, a.Confidence)
				}
			}
		}
		if !found {
			t.Errorf("Rect::%s not attributed to Rect", m)
		}
	}

	// `pub fn bounding(a: &Rect, b: &Rect) -> Rect` is a free function. C's
	// first-parameter rule would claim it for Rect; Rust must not.
	b := get("bounding")
	if b.Receiver() != "" {
		t.Fatalf("bounding has receiver %q", b.Receiver())
	}
	if b.Chosen != "" {
		t.Errorf("free function bounding was attached to %q", st.TypeName(b.Chosen))
	}

	// No language with syntactic methods should ever produce LLM work.
	if n := len(st.Residual()); n != 0 {
		t.Errorf("%d Rust functions were sent for adjudication; expected none", n)
	}
	for _, a := range st.Assocs {
		if a.Source == SourceRule {
			t.Errorf("%s was decided by a C heuristic", a.Fn.Name)
		}
	}
}

// The point of import resolution: two functions named `area` exist, and the
// call in lib.rs must reach the one that was actually imported.
func TestRustImportResolutionPicksTheImportedSymbol(t *testing.T) {
	d := rustDraft(t)

	byID := map[string]bool{}
	for _, b := range d.Buildings {
		byID[b.ID] = true
	}

	// `use crate::shapes::circle::{Circle, area as circle_area}` then
	// `circle_area(1.0)` must land on circle.rs, never on util.rs.
	var hitCircle, hitUtil bool
	for _, e := range d.Edges {
		if e.Source != "src/lib.rs::Canvas" {
			continue
		}
		switch e.Target {
		case "src/shapes/circle.rs::<module>":
			hitCircle = true
		case "src/util.rs::<module>":
			hitUtil = true
		}
	}
	if !hitCircle {
		t.Error("aliased import circle_area did not resolve to shapes::circle")
	}
	if hitUtil {
		// util::clamp is also called, so a util edge is legitimate; this only
		// fails if circle_area resolved there instead.
		if !hitCircle {
			t.Error("circle_area resolved to util::area")
		}
	}

	// `clamp` is imported from util and called; that edge must exist.
	foundClamp := false
	for _, e := range d.Edges {
		if e.Source == "src/lib.rs::Canvas" && e.Target == "src/util.rs::<module>" {
			foundClamp = true
		}
	}
	if !foundClamp {
		t.Error("call to the imported util::clamp produced no edge")
	}

	// `Rect::area(s)` is a qualified call on a type; it must reach Rect.
	foundRect := false
	for _, e := range d.Edges {
		if e.Source == "src/lib.rs::Canvas" && e.Target == "src/shapes/rect.rs::Rect" {
			foundRect = true
		}
	}
	if !foundRect {
		t.Error("qualified call Rect::area did not resolve to Rect")
	}
}

func TestRustTypesAndFields(t *testing.T) {
	d := rustDraft(t)
	byName := map[string]int{}
	fields := map[string]int{}
	methods := map[string]int{}
	for _, b := range d.Buildings {
		byName[b.Name]++
		fields[b.Name] = b.NumFields
		methods[b.Name] = b.NumMethods
	}

	if byName["Rect"] != 1 || byName["Circle"] != 1 || byName["Canvas"] != 1 {
		t.Errorf("expected one building each for Rect, Circle, Canvas; got %v", byName)
	}
	if fields["Rect"] != 4 {
		t.Errorf("Rect has %d fields, want 4", fields["Rect"])
	}
	if methods["Rect"] != 2 {
		t.Errorf("Rect has %d methods, want 2 (new, area)", methods["Rect"])
	}
	if methods["Canvas"] != 4 {
		t.Errorf("Canvas has %d methods, want 4", methods["Canvas"])
	}
	// Free functions become part of their file's module building.
	if methods["rect.rs"] != 1 {
		t.Errorf("rect.rs module has %d functions, want 1 (bounding)", methods["rect.rs"])
	}
}
