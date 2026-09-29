// Package lang holds everything that knows about a specific programming
// language. Nothing outside this package names a tree-sitter node type.
//
// A new language is added by implementing Language and registering it. The
// association ladder, call graph, clustering, layout and renderer downstream
// are untouched: they only ever see the neutral facts defined here.
package lang

import (
	"strings"

	sitter "github.com/smacker/go-tree-sitter"
)

// Ref is a reference to a symbol exactly as it was written at the call site.
//
// Splitting the qualifier from the name is what makes import resolution
// possible: `layout::compute()` and a bare `compute()` are different
// questions, and collapsing them to a bare name (as the first version did)
// throws away the only information that can tell two same-named symbols apart.
type Ref struct {
	// Qualifier is the path written before the name: "crate::layout", "pkg",
	// or "" when the reference is unqualified.
	Qualifier string
	Name      string
	// Method is true when the reference is a method call on a value
	// (`x.foo()`). The receiver's type is unknown without type inference, so
	// these resolve on name alone and rank below everything else.
	Method bool
}

func (r Ref) String() string {
	if r.Qualifier == "" {
		return r.Name
	}
	return r.Qualifier + "::" + r.Name
}

// Import is one import/include/use statement.
type Import struct {
	// Path as written: "crate::layout::Rect", "git2/oid.h", "os/exec".
	Path string
	// Alias is the local name when renamed (`use x as y`), else "".
	Alias string
	// Wildcard is true for `use x::*` and for C includes, which make every
	// symbol of the target visible rather than one named thing.
	Wildcard bool
}

// TypeDef is a concrete type definition: a struct, enum, union, class or trait
// that actually declares members. Forward declarations must not be reported.
type TypeDef struct {
	Name       string
	Fields     []string
	FieldTypes []string
	LOC        int
	// Span is the definition's byte range, so source can be quoted without
	// any language-specific knowledge of how a declaration is shaped.
	Start, End uint32
}

// FuncDef is a function or method.
type FuncDef struct {
	Name string
	// Receiver is the type this function is a method of, taken from syntax
	// where the language has it (Go receivers, Rust `impl` blocks, class
	// bodies). Empty for free functions, and always empty for C, which has no
	// syntactic notion of a method at all.
	Receiver   string
	Signature  string
	ReturnType string
	ParamTypes []string
	TypesUsed  []Ref
	Calls      []Ref
	LOC        int
	// Complexity is McCabe's cyclomatic complexity: one plus the number of
	// decision points. Each language counts its own branch constructs, since
	// only it knows what they are called.
	Complexity int
	Start, End uint32
}

// CountBranches adds one per decision point found in a subtree. `branch` names
// the language's branching constructs; `shortCircuit` names the binary
// operators that introduce a path (`&&`, `||`), which are counted by operator
// text rather than by node type because every grammar shares one node type for
// all binary expressions.
func CountBranches(n *sitter.Node, src []byte, branch map[string]bool, binaryNode string) int {
	count := 0
	Walk(n, func(x *sitter.Node) bool {
		t := x.Type()
		if branch[t] {
			count++
		}
		if t == binaryNode {
			if op := x.ChildByFieldName("operator"); op != nil {
				switch Text(op, src) {
				case "&&", "||":
					count++
				}
			}
		}
		return true
	})
	return count
}

// FileFacts is everything one file contributes.
type FileFacts struct {
	Imports  []Import
	Types    []TypeDef
	Funcs    []FuncDef
	ScopeVar int // file-scope variables, used to size "module" buildings
}

// Language is the plug-in point. Implement it, register it, done.
type Language interface {
	// Name is the display name, also used in LLM prompts.
	Name() string
	// Extensions are the file suffixes this language claims, including the dot.
	Extensions() []string
	Grammar() *sitter.Language

	// SyntacticMethods reports whether the language states method ownership
	// in its grammar (Rust `impl`, Go receivers, class bodies).
	//
	// When true, Receiver is the whole truth: a function without one is a
	// free function, and guessing an owner for it from parameter types would
	// invent a relationship the language explicitly does not have. Only
	// languages that cannot express the idea at all - C - need the heuristic
	// ladder and the model that adjudicates it.
	SyntacticMethods() bool

	// ModuleScoped reports whether an unqualified name resolves only within
	// the current module plus whatever was explicitly imported.
	//
	// C is not: one global namespace, so a bare call may reach anything. Rust
	// and Go are, and pretending otherwise makes a bare `new()` in one crate
	// resolve into any other crate that happens to be imported - which fused
	// a third of ripgrep into one false dependency cycle.
	ModuleScoped() bool

	// Namespace is the scope a file's symbols belong to. Languages with a
	// module system return a module path ("crate::layout"); languages with a
	// single global namespace return the file path, which still lets the
	// resolver prefer symbols the caller can actually see.
	Namespace(relPath string) string

	// Separator joins namespace segments ("::", ".", "/").
	Separator() string

	// ImportTargets expands one import into namespace candidates, best first.
	// They are matched against known namespaces exactly, then by suffix.
	// `from` is the importing file's namespace, needed for relative imports
	// such as Rust's `super::`.
	ImportTargets(imp Import, from string) []string

	// Parse extracts facts from one parsed file.
	Parse(root *sitter.Node, src []byte) FileFacts
}

var registry []Language

// Register makes a language available. Called from each implementation's init.
func Register(l Language) { registry = append(registry, l) }

// All returns every registered language.
func All() []Language { return registry }

// ForFile picks the language that claims a path, or nil.
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

// Extensions lists every claimed suffix, for the directory walk.
func Extensions() map[string]bool {
	out := make(map[string]bool)
	for _, l := range registry {
		for _, e := range l.Extensions() {
			out[e] = true
		}
	}
	return out
}

// ---- helpers shared by implementations ----

// Text returns a node's source text.
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

// Lines counts the rows a node spans.
func Lines(n *sitter.Node) int {
	return int(n.EndPoint().Row-n.StartPoint().Row) + 1
}

// Collapse squeezes whitespace onto one line, for signatures.
func Collapse(s string) string {
	return strings.Join(strings.Fields(strings.ReplaceAll(s, "\n", " ")), " ")
}

// Children iterates a node's direct children.
func Children(n *sitter.Node, fn func(i int, ch *sitter.Node)) {
	for i := 0; i < int(n.ChildCount()); i++ {
		if ch := n.Child(i); ch != nil {
			fn(i, ch)
		}
	}
}

// Walk visits a subtree. Returning false from fn skips that node's children.
func Walk(n *sitter.Node, fn func(*sitter.Node) bool) {
	if !fn(n) {
		return
	}
	Children(n, func(_ int, ch *sitter.Node) { Walk(ch, fn) })
}
