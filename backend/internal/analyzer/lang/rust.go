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
// The last source root in the path separates crate from module. Treating a
// workspace as one module path makes every crate a sibling under `crate::`,
// where `printer::color` and `matcher::color` collide.
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
		// Not every member has src/; ripgrep's core crate does not.
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

// ImportTargets expands a `use`. The last segment is often a type rather than
// a module, so the parent path is offered too and the resolver picks.
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
		parent, _, ok := splitLast(from)
		if !ok {
			parent = "crate"
		}
		p = parent + "::" + strings.TrimPrefix(p, "super::")
	case p == "self":
		p = from
	case !strings.HasPrefix(p, "crate::") && p != "crate":
		// A 2015-style bare `use foo::bar`, or an external crate: offer the
		// crate-relative reading too, since an external one matches nothing.
		out := withParent(nil, p)
		// A workspace sibling is written as its package name, by convention
		// <project>_<crate>, while its namespace is just <crate>.
		head, rest, _ := strings.Cut(p, "::")
		if project, short, ok := strings.Cut(head, "_"); ok && project != "" && short != "" {
			out = append(out, short)
			if rest != "" {
				out = append(out, short+"::"+rest)
			}
		}
		return withParent(out, "crate::"+p)
	}

	return withParent(nil, p)
}

// withParent appends a path and, when it has one, its parent: the last
// segment of a `use` is often a type rather than a module.
func withParent(out []string, p string) []string {
	out = append(out, p)
	if parent, _, _ := splitLast(p); parent != "" {
		out = append(out, parent)
	}
	return out
}

func (rustLang) Parse(root *sitter.Node, src []byte) FileFacts {
	var out FileFacts

	// The enclosing impl names the receiver: what C guesses at with three
	// rules and an LLM is a field lookup here.
	var scan func(n *sitter.Node, receiver string)
	scan = func(n *sitter.Node, receiver string) {
		for ch := range Children(n) {
			switch ch.Type() {
			case "use_declaration":
				if a := ch.ChildByFieldName("argument"); a != nil {
					rustUse(a, src, "", &out.Imports)
				}
				continue

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
				continue

			case "impl_item":
				// `impl Trait for Type` attributes to the concrete type.
				recv := Text(ch.ChildByFieldName("type"), src)
				recv = rustBareType(recv)
				if body := ch.ChildByFieldName("body"); body != nil {
					scan(body, recv)
				}
				continue

			case "function_item", "function_signature_item":
				if f := rustFunc(ch, src, receiver); f != nil {
					out.Funcs = append(out.Funcs, *f)
				}
				continue

			case "mod_item":
				// Inline `mod x { ... }`: its contents live in this file, so
				// this file's namespace is the right answer for the common case.
				if body := ch.ChildByFieldName("body"); body != nil {
					scan(body, receiver)
				}
				continue

			case "static_item", "const_item":
				out.ScopeVar++
				continue
			}
			scan(ch, receiver)
		}
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
		// `use a::b::c` brings `c` into scope; the implicit alias is what
		// lets a later bare `c()` find it.
		_, last, _ := splitLast(full)
		*out = append(*out, Import{Path: full, Alias: last})

	case "use_as_clause":
		p := Text(n.ChildByFieldName("path"), src)
		alias := Text(n.ChildByFieldName("alias"), src)
		*out = append(*out, Import{Path: join(prefix, p), Alias: alias})

	case "use_wildcard":
		p := ""
		for ch := range Children(n) {
			if ch.Type() != "*" {
				p = Text(ch, src) // the last non-* child is the path
			}
		}
		*out = append(*out, Import{Path: join(prefix, p), Wildcard: true})

	case "scoped_use_list":
		p := join(prefix, Text(n.ChildByFieldName("path"), src))
		if list := n.ChildByFieldName("list"); list != nil {
			rustUseList(list, src, p, out)
		}

	case "use_list":
		rustUseList(n, src, prefix, out)
	}
}

func rustUseList(list *sitter.Node, src []byte, prefix string, out *[]Import) {
	for ch := range Children(list) {
		if ch.IsNamed() {
			rustUse(ch, src, prefix, out)
		}
	}
}

func rustTypeDef(n *sitter.Node, src []byte) *TypeDef {
	name := Text(n.ChildByFieldName("name"), src)
	if name == "" {
		return nil
	}
	body := n.ChildByFieldName("body")
	if body == nil {
		// No members: recording these repeats C's forward-declaration bug.
		return nil
	}

	fields := make([]string, 0, body.ChildCount())
	types := make([]string, 0, body.ChildCount())
	for ch := range Children(body) {
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
	}

	return newType(n, name, fields, types)
}

func rustFunc(n *sitter.Node, src []byte, receiver string) *FuncDef {
	name := Text(n.ChildByFieldName("name"), src)
	if name == "" {
		return nil
	}

	fn := newFunc(n, n.ChildByFieldName("parameters"), src, name, receiver, func(p *sitter.Node) (string, bool) {
		switch p.Type() {
		case "parameter":
			return rustTypeName(p.ChildByFieldName("type"), src), true
		case "self_parameter":
			return receiver, true
		}
		return "", false
	})
	fn.ReturnType = rustTypeName(n.ChildByFieldName("return_type"), src)
	fn.scanBody(n.ChildByFieldName("body"), src, rustBranchNodes, rustBodyRefs)
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
					// The receiver's type needs inference we do not do.
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
		for ch := range Children(t) {
			if !ch.IsNamed() {
				continue
			}
			if name := rustTypeName(ch, src); name != "" {
				return name
			}
		}
		return ""
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
