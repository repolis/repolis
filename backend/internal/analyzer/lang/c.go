package lang

import (
	"strings"

	sitter "github.com/smacker/go-tree-sitter"
	tsc "github.com/smacker/go-tree-sitter/c"
)

func init() { Register(&cLang{}) }

type cLang struct{}

func (cLang) Name() string              { return "C" }
func (cLang) Extensions() []string      { return []string{".c", ".h"} }
func (cLang) Grammar() *sitter.Language { return tsc.GetLanguage() }
func (cLang) Separator() string         { return "" }

// C cannot express method ownership at all, which is why the heuristic
// ladder and its LLM adjudicator exist.
func (cLang) SyntacticMethods() bool { return false }
func (cLang) ModuleScoped() bool     { return false }

// Namespace: C has one global namespace, so the file itself is the scope. It
// carries no meaning for name lookup, but it lets the resolver rank a symbol
// the caller actually includes above an identically named one it does not.
func (cLang) Namespace(relPath string) string { return relPath }

// ImportTargets: an #include names a file. Namespaces are file paths, so a
// suffix match resolves `#include "git2/oid.h"` to `include/git2/oid.h`.
func (cLang) ImportTargets(imp Import, _ string) []string {
	if imp.Path == "" {
		return nil
	}
	return []string{imp.Path}
}

func (l cLang) Parse(root *sitter.Node, src []byte) FileFacts {
	var out FileFacts
	seen := make(map[uint32]bool)

	Walk(root, func(n *sitter.Node) bool {
		if seen[n.StartByte()] {
			return false
		}
		switch n.Type() {
		case "struct_specifier", "union_specifier", "enum_specifier":
			if t := cTypeDef(n, src, ""); t != nil {
				out.Types = append(out.Types, *t)
			}

		case "type_definition":
			if t := cTypedef(n, src); t != nil {
				out.Types = append(out.Types, *t)
				Children(n, func(_ int, ch *sitter.Node) {
					if cIsTypeSpecifier(ch.Type()) {
						seen[ch.StartByte()] = true
					}
				})
			}

		case "function_definition":
			if f := cFunc(n, src); f != nil {
				out.Funcs = append(out.Funcs, *f)
			}
			return false // body already consumed

		case "preproc_include":
			if p := cIncludePath(Text(n, src)); p != "" {
				out.Imports = append(out.Imports, Import{Path: p, Wildcard: true})
			}

		case "declaration":
			if n.Parent() != nil && n.Parent().Type() == "translation_unit" {
				out.ScopeVar++
			}
		}
		return true
	})

	return out
}

func cIsTypeSpecifier(t string) bool {
	return t == "struct_specifier" || t == "union_specifier" || t == "enum_specifier"
}

// cBodyList returns the member list, or nil for a forward declaration or a
// bare type reference. `struct Level *p;` also contains a struct_specifier;
// recording those as definitions is what used to overwrite the real ones.
func cBodyList(n *sitter.Node) *sitter.Node {
	if b := n.ChildByFieldName("body"); b != nil {
		return b
	}
	var found *sitter.Node
	Children(n, func(_ int, ch *sitter.Node) {
		if found == nil && (ch.Type() == "field_declaration_list" || ch.Type() == "enumerator_list") {
			found = ch
		}
	})
	return found
}

func cTypeDef(n *sitter.Node, src []byte, nameOverride string) *TypeDef {
	body := cBodyList(n)
	if body == nil {
		return nil
	}
	name := nameOverride
	if name == "" {
		name = Text(n.ChildByFieldName("name"), src)
	}
	if name == "" {
		return nil
	}
	fields, types := cMembers(body, src)
	return &TypeDef{
		Name: name, Fields: fields, FieldTypes: types, LOC: Lines(n),
		Start: n.StartByte(), End: n.EndByte(),
	}
}

func cTypedef(n *sitter.Node, src []byte) *TypeDef {
	var inner *sitter.Node
	Children(n, func(_ int, ch *sitter.Node) {
		if inner == nil && cIsTypeSpecifier(ch.Type()) {
			inner = ch
		}
	})
	if inner == nil {
		return nil
	}
	name := strings.TrimLeft(Text(n.ChildByFieldName("declarator"), src), "* \t")
	return cTypeDef(inner, src, name)
}

func cMembers(body *sitter.Node, src []byte) ([]string, []string) {
	fields := make([]string, 0, body.ChildCount())
	types := make([]string, 0, body.ChildCount())
	Children(body, func(_ int, ch *sitter.Node) {
		switch ch.Type() {
		case "field_declaration":
			// `int x, y;` is one declaration with two declarators; reading
			// only the first undercounts the footprint of every struct that
			// uses the comma form.
			fields = append(fields, cDeclaratorNames(ch, src)...)
			if t := cBaseType(ch.ChildByFieldName("type"), src); t != "" {
				types = append(types, t)
			}
		case "enumerator":
			if id := ch.ChildByFieldName("name"); id != nil {
				fields = append(fields, Text(id, src))
			}
		}
	})
	return fields, types
}

func cDeclaratorNames(n *sitter.Node, src []byte) []string {
	var out []string
	for i := 0; i < int(n.ChildCount()); i++ {
		if n.FieldNameForChild(i) != "declarator" {
			continue
		}
		if ch := n.Child(i); ch != nil {
			if name := cUnwrap(ch, src); name != "" {
				out = append(out, name)
			}
		}
	}
	return out
}

func cUnwrap(n *sitter.Node, src []byte) string {
	switch n.Type() {
	case "field_identifier", "identifier", "type_identifier":
		return Text(n, src)
	case "pointer_declarator", "array_declarator", "function_declarator",
		"parenthesized_declarator", "init_declarator":
		if d := n.ChildByFieldName("declarator"); d != nil {
			return cUnwrap(d, src)
		}
		var r string
		Children(n, func(_ int, ch *sitter.Node) {
			if r == "" {
				r = cUnwrap(ch, src)
			}
		})
		return r
	}
	return ""
}

// cBaseType reduces a type node to a user-defined name, "" for primitives.
func cBaseType(t *sitter.Node, src []byte) string {
	if t == nil {
		return ""
	}
	switch t.Type() {
	case "type_identifier":
		return Text(t, src)
	case "struct_specifier", "union_specifier", "enum_specifier":
		return Text(t.ChildByFieldName("name"), src)
	}
	return ""
}

func cFunc(node *sitter.Node, src []byte) *FuncDef {
	decl := node.ChildByFieldName("declarator")
	if decl == nil {
		return nil
	}
	fnDecl := cFuncDeclarator(decl)
	if fnDecl == nil {
		return nil
	}
	name := cUnwrap(fnDecl.ChildByFieldName("declarator"), src)
	if name == "" {
		return nil
	}

	var paramTypes []string
	signature := ""
	if params := fnDecl.ChildByFieldName("parameters"); params != nil {
		signature = Collapse(Text(params, src))
		Children(params, func(_ int, p *sitter.Node) {
			if p.Type() == "parameter_declaration" {
				paramTypes = append(paramTypes, cBaseType(p.ChildByFieldName("type"), src))
			}
		})
	}

	fn := &FuncDef{
		Name:       name,
		Signature:  signature,
		ReturnType: cBaseType(node.ChildByFieldName("type"), src),
		ParamTypes: paramTypes,
		LOC:        Lines(node),
		Start:      node.StartByte(),
		End:        node.EndByte(),
		// C has no syntactic receiver. That absence is the entire reason the
		// association ladder exists; every other supported language fills
		// Receiver in directly.
		Receiver: "",
	}
	if body := node.ChildByFieldName("body"); body != nil {
		cBodyRefs(body, src, fn)
		fn.Complexity = 1 + CountBranches(body, src, cBranchNodes, "binary_expression")
	}
	return fn
}

var cBranchNodes = map[string]bool{
	"if_statement": true, "for_statement": true, "while_statement": true,
	"do_statement": true, "case_statement": true, "conditional_expression": true,
	"goto_statement": true,
}

func cFuncDeclarator(n *sitter.Node) *sitter.Node {
	switch n.Type() {
	case "function_declarator":
		return n
	case "pointer_declarator", "parenthesized_declarator":
		if d := n.ChildByFieldName("declarator"); d != nil {
			return cFuncDeclarator(d)
		}
	}
	var found *sitter.Node
	Children(n, func(_ int, ch *sitter.Node) {
		if found == nil && ch.Type() == "function_declarator" {
			found = ch
		}
	})
	return found
}

func cBodyRefs(n *sitter.Node, src []byte, fn *FuncDef) {
	Walk(n, func(x *sitter.Node) bool {
		switch x.Type() {
		case "call_expression":
			if f := x.ChildByFieldName("function"); f != nil {
				switch f.Type() {
				case "identifier":
					fn.Calls = append(fn.Calls, Ref{Name: Text(f, src)})
				case "field_expression":
					// p->handler(...): C's vtable idiom. The member name often
					// matches a real function, but there is no type to qualify
					// it with, so it resolves on name alone.
					if fld := f.ChildByFieldName("field"); fld != nil {
						fn.Calls = append(fn.Calls, Ref{Name: Text(fld, src), Method: true})
					}
				}
			}
		case "type_identifier":
			fn.TypesUsed = append(fn.TypesUsed, Ref{Name: Text(x, src)})
		case "struct_specifier", "union_specifier", "enum_specifier":
			if id := x.ChildByFieldName("name"); id != nil {
				fn.TypesUsed = append(fn.TypesUsed, Ref{Name: Text(id, src)})
			}
		}
		return true
	})
}

func cIncludePath(include string) string {
	if i := strings.Index(include, `"`); i != -1 {
		if e := strings.Index(include[i+1:], `"`); e != -1 {
			return include[i+1 : i+1+e]
		}
	}
	if i := strings.Index(include, "<"); i != -1 {
		if e := strings.Index(include[i+1:], ">"); e != -1 {
			return include[i+1 : i+1+e]
		}
	}
	return ""
}
