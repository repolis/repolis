package lang

import (
	"iter"
	"strings"

	sitter "github.com/smacker/go-tree-sitter"
)

type Ref struct {
	Qualifier string // "crate::layout", "pkg", or "" when unqualified
	Name      string
	// `x.foo()`: the receiver's type is unknown, so these resolve on name alone.
	Method bool
}

func (r Ref) String() string {
	if r.Qualifier == "" {
		return r.Name
	}
	return r.Qualifier + "::" + r.Name
}

type Import struct {
	Path  string // as written: "crate::layout::Rect", "git2/oid.h"
	Alias string // local name when renamed, else ""
	// `use x::*` and C includes, which expose every symbol of the target.
	Wildcard bool
}

// Only types that declare members; forward declarations must not be reported.
type TypeDef struct {
	Name       string
	Fields     []string
	FieldTypes []string
	LOC        int
	Start, End uint32 // byte range, for quoting source
}

type FuncDef struct {
	Name string
	// From syntax only; always empty for C.
	Receiver   string
	Signature  string
	ReturnType string
	ParamTypes []string
	TypesUsed  []Ref
	Calls      []Ref
	LOC        int
	Complexity int // McCabe: one plus the number of decision points
	Start, End uint32
}

type FileFacts struct {
	Imports  []Import
	Types    []TypeDef
	Funcs    []FuncDef
	ScopeVar int // file-scope variables, used to size "module" buildings
}

type Language interface {
	// Display name, also used in LLM prompts.
	Name() string
	// File suffixes, including the dot.
	Extensions() []string
	Grammar() *sitter.Language

	// Does the grammar state method ownership (Rust `impl`, Go receivers)? If
	// so, a function without a Receiver is genuinely free; only C needs the
	// ladder.
	SyntacticMethods() bool

	// Does an unqualified name resolve only within the current module plus
	// explicit imports? C is one global namespace; Rust and Go are not.
	ModuleScoped() bool

	// The scope a file's symbols belong to: a module path, or the file path
	// for languages with one global namespace.
	Namespace(relPath string) string

	Separator() string // joins namespace segments: "::", ".", "/"

	// Expands one import into namespace candidates, best first, matched
	// exactly then by suffix. `from` supports relative imports like `super::`.
	ImportTargets(imp Import, from string) []string

	Parse(root *sitter.Node, src []byte) FileFacts
}

var registry []Language

func Register(l Language) { registry = append(registry, l) }

func ByName(name string) Language {
	for _, l := range registry {
		if l.Name() == name {
			return l
		}
	}
	return nil
}

func ForFile(relPath string) Language {
	lower := strings.ToLower(relPath)
	for _, l := range registry {
		for _, ext := range l.Extensions() {
			if strings.HasSuffix(lower, ext) {
				return l
			}
		}
	}
	return nil
}

func Text(n *sitter.Node, src []byte) string {
	if n == nil {
		return ""
	}
	s, e := n.StartByte(), n.EndByte()
	if s <= e && e <= uint32(len(src)) {
		return string(src[s:e])
	}
	return ""
}

func Lines(n *sitter.Node) int {
	return int(n.EndPoint().Row-n.StartPoint().Row) + 1
}

func Collapse(s string) string {
	return strings.Join(strings.Fields(strings.ReplaceAll(s, "\n", " ")), " ")
}

func Children(n *sitter.Node) iter.Seq[*sitter.Node] {
	return func(yield func(*sitter.Node) bool) {
		for i := range int(n.ChildCount()) {
			if ch := n.Child(i); ch != nil && !yield(ch) {
				return
			}
		}
	}
}

// A field can repeat: `int x, y;` is one declaration with two declarators.
func FieldChildren(n *sitter.Node, field string) iter.Seq[*sitter.Node] {
	return func(yield func(*sitter.Node) bool) {
		for i := range int(n.ChildCount()) {
			if n.FieldNameForChild(i) != field {
				continue
			}
			if ch := n.Child(i); ch != nil && !yield(ch) {
				return
			}
		}
	}
}

// Walk visits a subtree. Returning false from fn skips that node's children.
func Walk(n *sitter.Node, fn func(*sitter.Node) bool) {
	if !fn(n) {
		return
	}
	for ch := range Children(n) {
		Walk(ch, fn)
	}
}

func splitLast(p string) (parent, last string, ok bool) {
	i := strings.LastIndex(p, "::")
	if i < 0 {
		return "", p, false
	}
	return p[:i], p[i+2:], true
}

func newType(n *sitter.Node, name string, fields, types []string) *TypeDef {
	return &TypeDef{
		Name: name, Fields: fields, FieldTypes: types, LOC: Lines(n),
		Start: n.StartByte(), End: n.EndByte(),
	}
}

// paramType reports false for parameter-list children that are not parameters.
func newFunc(n, params *sitter.Node, src []byte, name, receiver string, paramType func(p *sitter.Node) (string, bool)) *FuncDef {
	fn := &FuncDef{
		Name: name, Receiver: receiver, LOC: Lines(n),
		Start: n.StartByte(), End: n.EndByte(),
	}
	if params != nil {
		fn.Signature = Collapse(Text(params, src))
		for p := range Children(params) {
			if t, ok := paramType(p); ok {
				fn.ParamTypes = append(fn.ParamTypes, t)
			}
		}
	}
	return fn
}

// `&&` and `||` match on operator text: the grammars share one node type for
// every binary expression.
func (fn *FuncDef) scanBody(body *sitter.Node, src []byte, branches map[string]bool, refs func(*sitter.Node, []byte, *FuncDef)) {
	if body == nil {
		return
	}
	refs(body, src, fn)
	fn.Complexity = 1
	Walk(body, func(x *sitter.Node) bool {
		if branches[x.Type()] {
			fn.Complexity++
		}
		if x.Type() == "binary_expression" {
			if op := Text(x.ChildByFieldName("operator"), src); op == "&&" || op == "||" {
				fn.Complexity++
			}
		}
		return true
	})
}
