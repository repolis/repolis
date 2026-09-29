package lang

import (
	"path"
	"strings"

	sitter "github.com/smacker/go-tree-sitter"
	tsrust "github.com/smacker/go-tree-sitter/rust"
)

func init() { Register(&rustLang{}) }

type rustLang struct{}

func (rustLang) Name() string              { return "Rust" }
func (rustLang) Extensions() []string      { return []string{".rs"} }
func (rustLang) Grammar() *sitter.Language { return tsrust.GetLanguage() }
func (rustLang) Separator() string         { return "::" }
func (rustLang) SyntacticMethods() bool    { return true }
func (rustLang) ModuleScoped() bool        { return true }

// Namespace maps a file to its module path.
//
//	src/lib.rs, src/main.rs      -> crate
//	src/layout.rs                -> crate::layout
//	src/layout/mod.rs            -> crate::layout
//	crates/printer/src/lib.rs    -> printer
//	crates/printer/src/color.rs  -> printer::color
//
// A Cargo workspace holds many crates, each with its own root. Treating the
// whole tree as one module path made every crate a sibling under `crate::`,
// so `printer::color` and `matcher::color` would have been distinguishable
// only by accident. The last source root in the path separates the crate from
// the module.
func (rustLang) Namespace(relPath string) string {
	p := strings.TrimSuffix(path.Clean(relPath), ".rs")
	parts := strings.Split(p, "/")

	root := -1
	for i, seg := range parts {
		switch seg {
		case "src", "tests", "examples", "benches", "bin":
			root = i
		}
	}

	crate := "crate"
	var mod []string
	switch {
	case root >= 0:
		// The segment just before the source root names the crate.
		if root > 0 {
			crate = parts[root-1]
		}
		mod = parts[root+1:]
	default:
		// Not every workspace member has a src/ directory; ripgrep's core
		// crate puts its modules directly under crates/core/.
		mod = parts
		for i, seg := range parts {
			if (seg == "crates" || seg == "packages") && i+1 < len(parts) {
				crate = parts[i+1]
				mod = parts[i+2:]
				break
			}
		}
	}

	// A crate root or a directory module adds no segment of its own.
	if len(mod) > 0 {
		switch mod[len(mod)-1] {
		case "lib", "main", "mod":
			mod = mod[:len(mod)-1]
		}
	}

	if len(mod) == 0 {
		return crate
	}
	return crate + "::" + strings.Join(mod, "::")
}

// ImportTargets expands a `use`. The last segment of a path is often a type or
// function rather than a module, so both the full path and its parent are
// offered; the resolver takes whichever matches a real namespace.
func (rustLang) ImportTargets(imp Import, from string) []string {
	p := imp.Path
	if p == "" {
		return nil
	}

	// Relative forms are resolved against the importing module.
	switch {
	case strings.HasPrefix(p, "self::"):
		p = from + "::" + strings.TrimPrefix(p, "self::")
	case strings.HasPrefix(p, "super::"):
		parent := from
		if i := strings.LastIndex(parent, "::"); i != -1 {
			parent = parent[:i]
		} else {
			parent = "crate"
		}
		p = parent + "::" + strings.TrimPrefix(p, "super::")
	case p == "self":
		p = from
	case !strings.HasPrefix(p, "crate::") && p != "crate":
		// A bare `use foo::bar` inside a 2015-style crate, or an external
		// crate. Offer the crate-relative reading too; an external crate
		// simply will not match any known namespace and is dropped.
		out := []string{p}
		if parent := trimLast(p); parent != "" {
			out = append(out, parent)
		}
		// A workspace sibling is written with its Cargo package name, which
		// is conventionally <project>_<crate> while the directory - and so
		// the namespace - is just <crate>. Offer that reading too, or every
		// cross-crate edge in a workspace is lost.
		if head, rest := splitHead(p); strings.Contains(head, "_") {
			if i := strings.Index(head, "_"); i > 0 && i+1 < len(head) {
				short := head[i+1:]
				out = append(out, short)
				if rest != "" {
					out = append(out, short+"::"+rest)
				}
			}
		}
		out = append(out, "crate::"+p)
		if parent := trimLast("crate::" + p); parent != "" {
			out = append(out, parent)
		}
		return out
	}

	out := []string{p}
	if parent := trimLast(p); parent != "" {
		out = append(out, parent)
	}
	return out
}

func lastRustSegment(p string) string {
	if i := strings.LastIndex(p, "::"); i != -1 {
		return p[i+2:]
	}
	return p
}

func splitHead(p string) (string, string) {
	if i := strings.Index(p, "::"); i > 0 {
		return p[:i], p[i+2:]
	}
	return p, ""
}

func trimLast(p string) string {
	if i := strings.LastIndex(p, "::"); i > 0 {
		return p[:i]
	}
	return ""
}

func (rustLang) Parse(root *sitter.Node, src []byte) FileFacts {
	var out FileFacts

	// `impl Camera { fn frame(&self) }` — the enclosing impl names the
	// receiver. This is the whole reason Rust needs no association heuristics:
	// what C guesses at with three rules and an LLM is here a field lookup.
	var scan func(n *sitter.Node, receiver string)
	scan = func(n *sitter.Node, receiver string) {
		Children(n, func(_ int, ch *sitter.Node) {
			switch ch.Type() {
			case "use_declaration":
				if a := ch.ChildByFieldName("argument"); a != nil {
					rustUse(a, src, "", &out.Imports)
				}
				return

			case "struct_item", "enum_item", "union_item", "trait_item":
				if t := rustTypeDef(ch, src); t != nil {
					out.Types = append(out.Types, *t)
				}
				// A trait body holds signatures that belong to the trait.
				if ch.Type() == "trait_item" {
					if body := ch.ChildByFieldName("body"); body != nil {
						scan(body, Text(ch.ChildByFieldName("name"), src))
					}
				}
				return

			case "impl_item":
				// `impl Trait for Type` attributes to the concrete type.
				recv := Text(ch.ChildByFieldName("type"), src)
				recv = rustBareType(recv)
				if body := ch.ChildByFieldName("body"); body != nil {
					scan(body, recv)
				}
				return

			case "function_item", "function_signature_item":
				if f := rustFunc(ch, src, receiver); f != nil {
					out.Funcs = append(out.Funcs, *f)
				}
				return

			case "mod_item":
				// Inline `mod x { ... }`. Its contents still live in this file;
				// treating them as this file's namespace keeps resolution
				// simple and is right for the common one-module-per-file case.
				if body := ch.ChildByFieldName("body"); body != nil {
					scan(body, receiver)
				}
				return

			case "static_item", "const_item":
				out.ScopeVar++
				return
			}
			scan(ch, receiver)
		})
	}
	scan(root, "")
	return out
}

// rustUse flattens one use-tree into flat imports.
func rustUse(n *sitter.Node, src []byte, prefix string, out *[]Import) {
	join := func(a, b string) string {
		if a == "" {
			return b
		}
		return a + "::" + b
	}

	switch n.Type() {
	case "identifier", "scoped_identifier", "crate", "super", "self":
		full := join(prefix, Text(n, src))
		// `use a::b::c` brings the name `c` into scope; recording it as an
		// implicit alias is what lets a later bare `c()` find it.
		*out = append(*out, Import{Path: full, Alias: lastRustSegment(full)})

	case "use_as_clause":
		p := Text(n.ChildByFieldName("path"), src)
		alias := Text(n.ChildByFieldName("alias"), src)
		*out = append(*out, Import{Path: join(prefix, p), Alias: alias})

	case "use_wildcard":
		p := ""
		Children(n, func(_ int, ch *sitter.Node) {
			if ch.Type() != "*" {
				p = Text(ch, src)
			}
		})
		*out = append(*out, Import{Path: join(prefix, p), Wildcard: true})

	case "scoped_use_list":
		p := join(prefix, Text(n.ChildByFieldName("path"), src))
		if list := n.ChildByFieldName("list"); list != nil {
			Children(list, func(_ int, ch *sitter.Node) {
				if ch.IsNamed() {
					rustUse(ch, src, p, out)
				}
			})
		}

	case "use_list":
		Children(n, func(_ int, ch *sitter.Node) {
			if ch.IsNamed() {
				rustUse(ch, src, prefix, out)
			}
		})
	}
}

func rustTypeDef(n *sitter.Node, src []byte) *TypeDef {
	name := Text(n.ChildByFieldName("name"), src)
	if name == "" {
		return nil
	}
	body := n.ChildByFieldName("body")
	if body == nil {
		// `struct Marker;` or `type Alias = ...` declare no members. Recording
		// them would reproduce C's forward-declaration problem.
		return nil
	}

	fields := make([]string, 0, body.ChildCount())
	types := make([]string, 0, body.ChildCount())
	Children(body, func(_ int, ch *sitter.Node) {
		switch ch.Type() {
		case "field_declaration":
			if id := ch.ChildByFieldName("name"); id != nil {
				fields = append(fields, Text(id, src))
			}
			if t := rustTypeName(ch.ChildByFieldName("type"), src); t != "" {
				types = append(types, t)
			}
		case "enum_variant":
			if id := ch.ChildByFieldName("name"); id != nil {
				fields = append(fields, Text(id, src))
			}
		case "function_signature_item", "function_item":
			// Trait methods count as members of the trait.
			if id := ch.ChildByFieldName("name"); id != nil {
				fields = append(fields, Text(id, src))
			}
		}
	})

	return &TypeDef{
		Name: name, Fields: fields, FieldTypes: types, LOC: Lines(n),
		Start: n.StartByte(), End: n.EndByte(),
	}
}

func rustFunc(n *sitter.Node, src []byte, receiver string) *FuncDef {
	name := Text(n.ChildByFieldName("name"), src)
	if name == "" {
		return nil
	}

	var paramTypes []string
	signature := ""
	if params := n.ChildByFieldName("parameters"); params != nil {
		signature = Collapse(Text(params, src))
		Children(params, func(_ int, p *sitter.Node) {
			switch p.Type() {
			case "parameter":
				paramTypes = append(paramTypes, rustTypeName(p.ChildByFieldName("type"), src))
			case "self_parameter":
				paramTypes = append(paramTypes, receiver)
			}
		})
	}

	fn := &FuncDef{
		Name:       name,
		Receiver:   receiver,
		Signature:  signature,
		ReturnType: rustTypeName(n.ChildByFieldName("return_type"), src),
		ParamTypes: paramTypes,
		LOC:        Lines(n),
		Start:      n.StartByte(),
		End:        n.EndByte(),
	}
	if body := n.ChildByFieldName("body"); body != nil {
		rustBodyRefs(body, src, fn)
		fn.Complexity = 1 + CountBranches(body, src, rustBranchNodes, "binary_expression")
	}
	return fn
}

var rustBranchNodes = map[string]bool{
	"if_expression": true, "while_expression": true, "loop_expression": true,
	"for_expression": true, "match_arm": true, "if_let_expression": true,
	"while_let_expression": true, "try_expression": true,
}

func rustBodyRefs(n *sitter.Node, src []byte, fn *FuncDef) {
	Walk(n, func(x *sitter.Node) bool {
		switch x.Type() {
		case "call_expression":
			if f := x.ChildByFieldName("function"); f != nil {
				switch f.Type() {
				case "identifier":
					fn.Calls = append(fn.Calls, Ref{Name: Text(f, src)})
				case "scoped_identifier":
					fn.Calls = append(fn.Calls, Ref{
						Qualifier: Text(f.ChildByFieldName("path"), src),
						Name:      Text(f.ChildByFieldName("name"), src),
					})
				case "field_expression":
					// `x.foo()` — the receiver's type needs inference we do
					// not do, so this resolves on name alone.
					if fld := f.ChildByFieldName("field"); fld != nil {
						fn.Calls = append(fn.Calls, Ref{Name: Text(fld, src), Method: true})
					}
				}
			}
		case "type_identifier":
			fn.TypesUsed = append(fn.TypesUsed, Ref{Name: Text(x, src)})
		case "scoped_type_identifier":
			fn.TypesUsed = append(fn.TypesUsed, Ref{
				Qualifier: Text(x.ChildByFieldName("path"), src),
				Name:      Text(x.ChildByFieldName("name"), src),
			})
			return false
		case "struct_expression":
			if nm := x.ChildByFieldName("name"); nm != nil {
				fn.TypesUsed = append(fn.TypesUsed, Ref{Name: rustBareType(Text(nm, src))})
			}
		}
		return true
	})
}

// rustTypeName reduces a type node to a bare user-defined name, "" for
// primitives. `&mut Vec<Rect>` yields "Vec".
func rustTypeName(t *sitter.Node, src []byte) string {
	if t == nil {
		return ""
	}
	switch t.Type() {
	case "type_identifier":
		return Text(t, src)
	case "primitive_type":
		return ""
	case "reference_type", "pointer_type", "array_type", "slice_type":
		return rustTypeName(t.ChildByFieldName("type"), src)
	case "generic_type":
		return rustTypeName(t.ChildByFieldName("type"), src)
	case "scoped_type_identifier":
		return Text(t.ChildByFieldName("name"), src)
	case "tuple_type":
		var first string
		Children(t, func(_ int, ch *sitter.Node) {
			if first == "" && ch.IsNamed() {
				first = rustTypeName(ch, src)
			}
		})
		return first
	}
	return rustBareType(Text(t, src))
}

// rustBareType strips generics, references and paths from written type text.
func rustBareType(s string) string {
	s = strings.TrimSpace(s)
	s = strings.TrimPrefix(s, "&")
	s = strings.TrimSpace(strings.TrimPrefix(s, "mut "))
	if i := strings.Index(s, "<"); i != -1 {
		s = s[:i]
	}
	if i := strings.LastIndex(s, "::"); i != -1 {
		s = s[i+2:]
	}
	return strings.TrimSpace(s)
}
