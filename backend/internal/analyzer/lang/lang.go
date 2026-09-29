// Package lang holds everything that knows about a specific language. Nothing
// outside it names a tree-sitter node type; adding a language means
// implementing Language and registering it.
package lang

import (
	"strings"

	sitter "github.com/smacker/go-tree-sitter"
)

// Ref is a symbol reference as written. The qualifier is kept separate because
// `layout::compute()` and a bare `compute()` resolve differently.
type Ref struct {
	Qualifier string // "crate::layout", "pkg", or "" when unqualified
	Name      string
	// A method call on a value (`x.foo()`): the receiver's type is unknown,
	// so these resolve on name alone.
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
	Path  string // as written: "crate::layout::Rect", "git2/oid.h"
	Alias string // local name when renamed, else ""
	// `use x::*` and C includes, which expose every symbol of the target.
	Wildcard bool
}

// TypeDef is a type that actually declares members. Forward declarations must
// not be reported.
type TypeDef struct {
	Name       string
	Fields     []string
	FieldTypes []string
	LOC        int
	Start, End uint32 // byte range, for quoting source
}

// FuncDef is a function or method.
type FuncDef struct {
	Name string
	// The type this is a method of, from syntax where the language has it.
	// Always empty for C, which cannot express the idea.
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

// CountBranches counts decision points. `&&` and `||` are matched on operator
// text because grammars share one node type for all binary expressions.
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

	// Does the grammar state method ownership (Rust `impl`, Go receivers)?
	// When true, a function without a Receiver is genuinely free and nothing
	// may be guessed. Only C, which cannot express the idea, needs the ladder.
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

func Lines(n *sitter.Node) int {
	return int(n.EndPoint().Row-n.StartPoint().Row) + 1
}

// Collapse squeezes whitespace onto one line, for signatures.
func Collapse(s string) string {
	return strings.Join(strings.Fields(strings.ReplaceAll(s, "\n", " ")), " ")
}

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
