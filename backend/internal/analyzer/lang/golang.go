package lang

import (
	"path"
	"strings"

	sitter "github.com/smacker/go-tree-sitter"
	tsgo "github.com/smacker/go-tree-sitter/golang"
)

func init() { Register(&goLang{}) }

type goLang struct{}

func (goLang) Name() string              { return "Go" }
func (goLang) Extensions() []string      { return []string{".go"} }
func (goLang) Grammar() *sitter.Language { return tsgo.GetLanguage() }
func (goLang) Separator() string         { return "." }

// A receiver states the owning type, so nothing has to be guessed.
func (goLang) SyntacticMethods() bool { return true }
func (goLang) ModuleScoped() bool     { return true }

// Namespace: a Go package is a directory, so the directory is the scope.
func (goLang) Namespace(relPath string) string {
	d := path.Dir(path.Clean(relPath))
	if d == "." || d == "/" || d == "" {
		return "root"
	}
	return d
}

// ImportTargets turns a Go import path into namespace candidates.
//
// Imports are absolute module paths ("github.com/owner/repo/internal/x") while
// namespaces are repository-relative directories ("internal/x"), so the path
// is offered with successively fewer leading segments and the longest match
// wins.
//
// A first segment without a dot means the standard library. Returning nothing
// for those matters: otherwise `import "os"` would match a repository's own
// `internal/os` by suffix and invent edges into it.
func (goLang) ImportTargets(imp Import, _ string) []string {
	p := strings.Trim(imp.Path, `"`)
	if p == "" {
		return nil
	}
	parts := strings.Split(p, "/")
	if !strings.Contains(parts[0], ".") {
		return nil // standard library
	}
	out := make([]string, 0, len(parts))
	for i := range parts {
		out = append(out, strings.Join(parts[i:], "/"))
	}
	return out
}

func (goLang) Parse(root *sitter.Node, src []byte) FileFacts {
	var out FileFacts

	Children(root, func(_ int, n *sitter.Node) {
		switch n.Type() {
		case "import_declaration":
			goImports(n, src, &out.Imports)

		case "type_declaration":
			Children(n, func(_ int, spec *sitter.Node) {
				// `type Alias = Camera` declares no members of its own;
				// recording it would duplicate the real type.
				if spec.Type() != "type_spec" {
					return
				}
				if t := goTypeDef(spec, src); t != nil {
					out.Types = append(out.Types, *t)
				}
			})

		case "function_declaration":
			if f := goFunc(n, src, ""); f != nil {
				out.Funcs = append(out.Funcs, *f)
			}

		case "method_declaration":
			if f := goFunc(n, src, goReceiverType(n, src)); f != nil {
				out.Funcs = append(out.Funcs, *f)
			}

		case "var_declaration", "const_declaration":
			out.ScopeVar++
		}
	})

	return out
}

func goImports(n *sitter.Node, src []byte, out *[]Import) {
	add := func(spec *sitter.Node) {
		raw := strings.Trim(Text(spec.ChildByFieldName("path"), src), `"`)
		if raw == "" {
			return
		}
		imp := Import{Path: raw}
		switch name := spec.ChildByFieldName("name"); {
		case name == nil:
			// The package name is conventionally the last path segment, and
			// that is what qualifies every call site, so it has to be
			// recorded as the alias even when nothing was renamed.
			imp.Alias = path.Base(raw)
		case name.Type() == "dot":
			imp.Wildcard = true
		case name.Type() == "blank_identifier":
			return // imported only for its side effects
		default:
			imp.Alias = Text(name, src)
		}
		*out = append(*out, imp)
	}

	Walk(n, func(x *sitter.Node) bool {
		if x.Type() == "import_spec" {
			add(x)
			return false
		}
		return true
	})
}

func goTypeDef(spec *sitter.Node, src []byte) *TypeDef {
	name := Text(spec.ChildByFieldName("name"), src)
	if name == "" {
		return nil
	}
	body := spec.ChildByFieldName("type")
	if body == nil {
		return nil
	}

	fields := make([]string, 0, 8)
	types := make([]string, 0, 8)

	switch body.Type() {
	case "struct_type":
		Walk(body, func(x *sitter.Node) bool {
			if x.Type() != "field_declaration" {
				return true
			}
			// `a, b int` is one declaration with two name fields.
			named := false
			for i := 0; i < int(x.ChildCount()); i++ {
				if x.FieldNameForChild(i) == "name" {
					if ch := x.Child(i); ch != nil {
						fields = append(fields, Text(ch, src))
						named = true
					}
				}
			}
			if t := goTypeName(x.ChildByFieldName("type"), src); t != "" {
				types = append(types, t)
				if !named {
					// An embedded field has no name; the type is the name.
					fields = append(fields, t)
				}
			}
			return false
		})

	case "interface_type":
		Walk(body, func(x *sitter.Node) bool {
			if x.Type() == "method_elem" {
				if id := x.ChildByFieldName("name"); id != nil {
					fields = append(fields, Text(id, src))
				}
				return false
			}
			return true
		})

	default:
		// `type Celsius float64` and friends define a named type with no
		// members. They can still own methods, so they are worth recording.
	}

	return &TypeDef{
		Name: name, Fields: fields, FieldTypes: types, LOC: Lines(spec),
		Start: spec.StartByte(), End: spec.EndByte(),
	}
}

// goReceiverType reads the owning type out of `func (c *Camera) Frame()`.
func goReceiverType(n *sitter.Node, src []byte) string {
	recv := n.ChildByFieldName("receiver")
	if recv == nil {
		return ""
	}
	var name string
	Walk(recv, func(x *sitter.Node) bool {
		if name != "" {
			return false
		}
		if x.Type() == "parameter_declaration" {
			name = goTypeName(x.ChildByFieldName("type"), src)
			return false
		}
		return true
	})
	return name
}

func goFunc(n *sitter.Node, src []byte, receiver string) *FuncDef {
	name := Text(n.ChildByFieldName("name"), src)
	if name == "" {
		return nil
	}

	var paramTypes []string
	signature := ""
	if params := n.ChildByFieldName("parameters"); params != nil {
		signature = Collapse(Text(params, src))
		Children(params, func(_ int, p *sitter.Node) {
			if p.Type() == "parameter_declaration" {
				paramTypes = append(paramTypes, goTypeName(p.ChildByFieldName("type"), src))
			}
		})
	}

	fn := &FuncDef{
		Name:       name,
		Receiver:   receiver,
		Signature:  signature,
		ReturnType: goTypeName(n.ChildByFieldName("result"), src),
		ParamTypes: paramTypes,
		LOC:        Lines(n),
		Start:      n.StartByte(),
		End:        n.EndByte(),
	}
	if body := n.ChildByFieldName("body"); body != nil {
		goBodyRefs(body, src, fn)
		fn.Complexity = 1 + CountBranches(body, src, goBranchNodes, "binary_expression")
	}
	return fn
}

var goBranchNodes = map[string]bool{
	"if_statement": true, "for_statement": true, "expression_case": true,
	"type_case": true, "default_case": true, "communication_case": true,
	"select_statement": true,
}

func goBodyRefs(n *sitter.Node, src []byte, fn *FuncDef) {
	Walk(n, func(x *sitter.Node) bool {
		switch x.Type() {
		case "call_expression":
			f := x.ChildByFieldName("function")
			if f == nil {
				break
			}
			switch f.Type() {
			case "identifier":
				fn.Calls = append(fn.Calls, Ref{Name: Text(f, src)})
			case "selector_expression":
				field := Text(f.ChildByFieldName("field"), src)
				operand := f.ChildByFieldName("operand")
				// `pkg.Fn()` and `value.Method()` are the same shape. A plain
				// identifier operand might be a package, so it is offered as a
				// qualifier; if no import matches, resolution falls back to
				// the bare name, which is the method case.
				if operand != nil && operand.Type() == "identifier" {
					fn.Calls = append(fn.Calls, Ref{Qualifier: Text(operand, src), Name: field})
				} else {
					fn.Calls = append(fn.Calls, Ref{Name: field, Method: true})
				}
			}
		case "type_identifier":
			fn.TypesUsed = append(fn.TypesUsed, Ref{Name: Text(x, src)})
		case "qualified_type":
			fn.TypesUsed = append(fn.TypesUsed, Ref{
				Qualifier: Text(x.ChildByFieldName("package"), src),
				Name:      Text(x.ChildByFieldName("name"), src),
			})
			return false
		case "composite_literal":
			if t := x.ChildByFieldName("type"); t != nil {
				if n := goTypeName(t, src); n != "" {
					fn.TypesUsed = append(fn.TypesUsed, Ref{Name: n})
				}
			}
		}
		return true
	})
}

// goTypeName reduces a type node to a bare user-defined name, "" for builtins.
func goTypeName(t *sitter.Node, src []byte) string {
	if t == nil {
		return ""
	}
	switch t.Type() {
	case "type_identifier":
		name := Text(t, src)
		if goBuiltin[name] {
			return ""
		}
		return name
	case "pointer_type", "slice_type", "array_type", "parenthesized_type", "variadic_parameter_declaration":
		if inner := t.ChildByFieldName("type"); inner != nil {
			return goTypeName(inner, src)
		}
		var r string
		Children(t, func(_ int, ch *sitter.Node) {
			if r == "" && ch.IsNamed() {
				r = goTypeName(ch, src)
			}
		})
		return r
	case "map_type":
		return goTypeName(t.ChildByFieldName("value"), src)
	case "qualified_type":
		return Text(t.ChildByFieldName("name"), src)
	case "generic_type":
		return goTypeName(t.ChildByFieldName("type"), src)
	}
	return ""
}

var goBuiltin = map[string]bool{
	"bool": true, "string": true, "error": true, "any": true, "rune": true, "byte": true,
	"int": true, "int8": true, "int16": true, "int32": true, "int64": true,
	"uint": true, "uint8": true, "uint16": true, "uint32": true, "uint64": true, "uintptr": true,
	"float32": true, "float64": true, "complex64": true, "complex128": true,
}
