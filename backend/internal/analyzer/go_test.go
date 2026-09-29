package analyzer

import "testing"

const goFixture = "testdata/gofixture"

func goDraft(t *testing.T) *Draft {
	t.Helper()
	raw, err := ExtractRepository(goFixture)
	if err != nil {
		t.Fatalf("ExtractRepository: %v", err)
	}
	return BuildDraft(raw, nil, nil)
}

func TestGoNamespacesArePackages(t *testing.T) {
	raw, err := ExtractRepository(goFixture)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]string{
		"main.go":        "root",
		"store/store.go": "store",
		"geom/geom.go":   "geom",
		"util/util.go":   "util",
	}
	for _, f := range raw.Files {
		if w, ok := want[f.Path]; ok && f.Namespace != w {
			t.Errorf("%s namespace = %q, want %q", f.Path, f.Namespace, w)
		}
		if f.Language != "Go" {
			t.Errorf("%s language = %q, want Go", f.Path, f.Language)
		}
	}
}

// A receiver states the owner outright; a constructor returning *Store is
// still a free function, however much it looks like a method.
func TestGoAssociationIsSyntacticOnly(t *testing.T) {
	raw, err := ExtractRepository(goFixture)
	if err != nil {
		t.Fatal(err)
	}
	st := BuildSymbolTable(raw)

	byName := map[string]*Assoc{}
	for _, a := range st.Assocs {
		byName[a.Fn.Name] = a
	}

	// Pointer and value receivers both attribute to the same type.
	for _, m := range []string{"Run", "Stop"} {
		a := byName[m]
		if a == nil {
			t.Fatalf("%s not extracted", m)
		}
		if st.TypeName(a.Chosen) != "App" || a.Source != SourceSyntax {
			t.Errorf("%s -> %q via %s, want App via syntax", m, st.TypeName(a.Chosen), a.Source)
		}
	}
	for _, m := range []string{"Put", "Get"} {
		a := byName[m]
		if a == nil || st.TypeName(a.Chosen) != "Store" || a.Source != SourceSyntax {
			t.Errorf("%s not attributed to Store by syntax", m)
		}
	}

	// `func NewStore() *Store` has no receiver, so it owns nothing.
	if a := byName["NewStore"]; a == nil || a.Chosen != "" {
		t.Errorf("NewStore was attached to %q; Go says it is a free function",
			st.TypeName(byName["NewStore"].Chosen))
	}
	if a := byName["New"]; a == nil || a.Chosen != "" {
		t.Errorf("New was attached to %q", st.TypeName(byName["New"].Chosen))
	}

	if n := len(st.Residual()); n != 0 {
		t.Errorf("%d Go functions were queued for the model; expected none", n)
	}
	for _, a := range st.Assocs {
		if a.Source == SourceRule {
			t.Errorf("%s was decided by a C heuristic", a.Fn.Name)
		}
	}
}

// Two packages export `Area`. The call in main.go is qualified by an aliased
// import and must reach geom, never util.
func TestGoImportResolution(t *testing.T) {
	d := goDraft(t)

	edge := func(src, tgt string) bool {
		for _, e := range d.Edges {
			if e.Source == src && e.Target == tgt {
				return true
			}
		}
		return false
	}

	const app = "main.go::App"

	// geom exports nothing but Area. If `geo.Area(2.0)` had resolved to
	// util.Area - the same bare name, a different package - this edge would
	// not exist at all, so its presence is the disambiguation.
	if !edge(app, "geom/geom.go::<module>") {
		t.Error("aliased import `geo \"example.com/app/geom\"` resolved away from geom")
	}
	if !edge(app, "util/util.go::<module>") {
		t.Error("util.Clamp produced no edge")
	}
	if !edge(app, "store/store.go::Store") {
		t.Error("method call a.s.Put did not reach Store")
	}

	// main.go imports "fmt" and "os" and calls into both. The standard
	// library is not part of the repository, so neither may produce an edge -
	// and an import of "os" must not be suffix-matched onto some unrelated
	// local directory either.
	for _, e := range d.Edges {
		if e.Target == "" {
			t.Errorf("edge with empty target from %s", e.Source)
		}
		for _, bad := range []string{"fmt", "os"} {
			if e.Target == bad || e.Source == bad {
				t.Errorf("standard library package %q became a building", bad)
			}
		}
	}
}

func TestGoTypesAndFields(t *testing.T) {
	d := goDraft(t)
	fields := map[string]int{}
	methods := map[string]int{}
	kind := map[string]string{}
	for _, b := range d.Buildings {
		fields[b.Name] = b.NumFields
		methods[b.Name] = b.NumMethods
		kind[b.Name] = b.Kind
	}

	if fields["App"] != 3 {
		t.Errorf("App has %d fields, want 3", fields["App"])
	}
	if methods["App"] != 2 {
		t.Errorf("App has %d methods, want 2 (Run, Stop)", methods["App"])
	}
	if fields["Store"] != 2 {
		t.Errorf("Store has %d fields, want 2", fields["Store"])
	}
	if methods["Store"] != 2 {
		t.Errorf("Store has %d methods, want 2 (Put, Get)", methods["Store"])
	}
	// An interface's method set is its members.
	if fields["Runner"] != 2 {
		t.Errorf("Runner has %d members, want 2", fields["Runner"])
	}
	// Free functions land in their file's module building.
	if methods["main.go"] != 1 {
		t.Errorf("main.go module has %d functions, want 1 (New)", methods["main.go"])
	}
	if methods["store.go"] != 1 {
		t.Errorf("store.go module has %d functions, want 1 (NewStore)", methods["store.go"])
	}
}

// A method name is unique per type, not per file. Three stateless codecs each
// declaring Name and Ext used to collapse onto whichever was parsed last,
// because the function key was file+name.
func TestGoSameMethodNameOnSeveralTypes(t *testing.T) {
	d := goDraft(t)
	methods := map[string][]string{}
	for _, b := range d.Buildings {
		methods[b.Name] = b.Methods
	}
	for _, typ := range []string{"jsonCodec", "xmlCodec", "yamlCodec"} {
		got := methods[typ]
		if len(got) != 2 {
			t.Errorf("%s has %d methods %v, want 2 (Ext, Name)", typ, len(got), got)
		}
	}
}
