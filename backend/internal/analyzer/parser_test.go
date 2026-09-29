package analyzer

import (
	"os"
	"path/filepath"
	"testing"
)

const fixture = "testdata/fixture"

func extract(t *testing.T) *RawExtraction {
	t.Helper()
	raw, err := ExtractRepository(fixture)
	if err != nil {
		t.Fatalf("ExtractRepository: %v", err)
	}
	return raw
}

func structByName(raw *RawExtraction, name string) *RawStruct {
	for i := range raw.Structs {
		if raw.Structs[i].Name == name {
			return &raw.Structs[i]
		}
	}
	return nil
}

// A forward declaration and a bare type reference both contain a
// struct_specifier node in tree-sitter-c. Recording those as definitions is
// what previously overwrote real struct data with empty stubs, leaving ~30-40%
// of every generated city as zero-field, one-line placeholders.
func TestOnlyRealDefinitionsAreExtracted(t *testing.T) {
	raw := extract(t)

	w := structByName(raw, "Widget")
	if w == nil {
		t.Fatal("Widget definition not extracted")
	}
	if w.SourceFile != "widget.c" {
		t.Errorf("Widget came from %q, want widget.c (the definition, not the typedef in the header)", w.SourceFile)
	}
	if len(w.Fields) != 4 {
		t.Errorf("Widget has %d fields %v, want 4", len(w.Fields), w.Fields)
	}

	for _, s := range raw.Structs {
		if len(s.Fields) == 0 {
			t.Errorf("%s from %s has no members; forward declarations must not be recorded", s.Name, s.SourceFile)
		}
	}
}

func TestTypedefAndEnumNames(t *testing.T) {
	raw := extract(t)
	for _, want := range []string{"Rect", "Palette", "Mode"} {
		if structByName(raw, want) == nil {
			t.Errorf("type %q not extracted", want)
		}
	}
	if r := structByName(raw, "Rect"); r != nil && len(r.Fields) != 4 {
		t.Errorf("Rect has %d fields %v, want 4", len(r.Fields), r.Fields)
	}
}

func TestFunctionSignatureExtraction(t *testing.T) {
	raw := extract(t)
	byName := map[string]*RawFunction{}
	for i := range raw.Functions {
		byName[raw.Functions[i].Name] = &raw.Functions[i]
	}

	n := byName["widget_new"]
	if n == nil {
		t.Fatal("widget_new not extracted")
	}
	if n.ReturnType != "Widget" {
		t.Errorf("widget_new returns %q, want Widget", n.ReturnType)
	}

	f := byName["widget_free"]
	if f == nil || len(f.ParamTypes) == 0 || f.ParamTypes[0] != "Widget" {
		t.Errorf("widget_free first param type = %v, want Widget", f.ParamTypes)
	}

	// Call sites drive the dependency graph; they must be attributed to the
	// enclosing function, not to the file.
	m := byName["main"]
	if m == nil {
		t.Fatal("main not extracted")
	}
	found := false
	for _, c := range m.Calls {
		if c.Name == "widget_new" {
			found = true
		}
	}
	if !found {
		t.Errorf("main's calls = %v, want widget_new among them", m.Calls)
	}
}

func TestAssociationLadder(t *testing.T) {
	raw := extract(t)
	st := BuildSymbolTable(raw)

	get := func(name string) *Assoc {
		for _, a := range st.Assocs {
			if a.Fn.Name == name {
				return a
			}
		}
		t.Fatalf("no association recorded for %s", name)
		return nil
	}

	// Name prefix and first parameter agree: settled for free.
	if a := get("widget_free"); a.Confidence != ConfHigh || st.TypeName(a.Chosen) != "Widget" {
		t.Errorf("widget_free -> %q (%s), want Widget (high)", st.TypeName(a.Chosen), a.Confidence)
	}
	// Return type only.
	if a := get("rect_make"); st.TypeName(a.Chosen) != "Rect" {
		t.Errorf("rect_make -> %q, want Rect", st.TypeName(a.Chosen))
	}
	// Name says Widget, first parameter says Palette: genuinely ambiguous, so
	// it must reach the LLM — while still carrying a usable tentative answer,
	// so that a missing or slow model never loses the function.
	amb := get("widget_recolor")
	if amb.Confidence != ConfLow {
		t.Errorf("widget_recolor confidence = %s, want low", amb.Confidence)
	}
	if amb.Chosen == "" {
		t.Error("widget_recolor has no tentative answer; an unavailable LLM would drop it")
	}
	residual := st.Residual()
	inResidual := false
	for _, a := range residual {
		if a.Fn.Name == "widget_recolor" {
			inResidual = true
		}
	}
	if !inResidual {
		t.Error("widget_recolor is not offered to the LLM")
	}

	// No type fits: becomes part of the file's module building.
	if a := get("clamp"); a.Chosen != "" {
		t.Errorf("clamp -> %q, want no owner", st.TypeName(a.Chosen))
	}
}

// The verifier is what makes an LLM answer safe to accept.
func TestVerifierRejectsUnsupportedAttribution(t *testing.T) {
	raw := extract(t)
	st := BuildSymbolTable(raw)

	var clamp *Assoc
	for _, a := range st.Assocs {
		if a.Fn.Name == "clamp" {
			clamp = a
		}
	}
	if clamp == nil {
		t.Fatal("clamp not found")
	}
	// clamp mentions no type at all, so no attribution can be supported.
	if st.ApplyLLM(clamp, typeID("widget.c", "Widget")) {
		t.Error("verifier accepted an attribution with no AST evidence")
	}
	// An id outside the candidate list must be refused too.
	if st.ApplyLLM(clamp, "nonexistent.c::Ghost") {
		t.Error("verifier accepted a type that is not a candidate")
	}
}

func TestDraftAssemblesModulesAndEdges(t *testing.T) {
	raw := extract(t)
	d := BuildDraft(raw, nil, nil)

	var types, modules int
	byID := map[string]bool{}
	for _, b := range d.Buildings {
		if byID[b.ID] {
			t.Errorf("duplicate building id %q", b.ID)
		}
		byID[b.ID] = true
		if b.Kind == "module" {
			modules++
		} else {
			types++
		}
	}
	if types == 0 || modules == 0 {
		t.Errorf("got %d types and %d modules, want both", types, modules)
	}

	// Free functions must survive as a module rather than being discarded.
	if !byID[moduleID("widget.c")] {
		t.Error("widget.c has unattached functions but produced no module building")
	}

	// Edges reference building ids, and both ends must exist.
	for _, e := range d.Edges {
		if !byID[e.Source] || !byID[e.Target] {
			t.Errorf("edge %s -> %s references an unknown building", e.Source, e.Target)
		}
	}
}

func TestClusteringCoversEveryBuilding(t *testing.T) {
	raw := extract(t)
	d := BuildDraft(raw, nil, nil)

	seen := map[string]int{}
	for _, c := range d.Clusters {
		for _, id := range c.Buildings {
			seen[id]++
		}
	}
	for _, b := range d.Buildings {
		if seen[b.ID] != 1 {
			t.Errorf("building %s appears in %d clusters, want exactly 1", b.ID, seen[b.ID])
		}
	}
}

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
	for _, typ := range Typologies {
		if !IsTypology(typ) {
			t.Errorf("%q is in Typologies but IsTypology says otherwise", typ)
		}
	}
}

// A workspace member owns a licence and a build manifest exactly like a
// vendored library does. Treating that as vendoring silently deleted three of
// ripgrep's own crates from the city.
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
