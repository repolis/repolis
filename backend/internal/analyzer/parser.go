package analyzer

import (
	"bytes"
	"context"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	sitter "github.com/smacker/go-tree-sitter"
	"github.com/smacker/go-tree-sitter/c"
)

// RawStruct is a struct/type extracted from the AST before LLM enrichment.
type RawStruct struct {
	Name        string   `json:"name"`
	SourceFile  string   `json:"source_file"`
	Fields      []string `json:"fields"`
	LinesOfCode int      `json:"lines_of_code"`
	BodySnippet string   `json:"body_snippet"` // minified source for LLM context
}

// RawFunction is a function extracted from the AST before LLM association.
type RawFunction struct {
	Name        string `json:"name"`
	SourceFile  string `json:"source_file"`
	Signature   string `json:"signature"`   // return type + params
	BodySnippet string `json:"body_snippet"` // minified source for LLM context
	LinesOfCode int    `json:"lines_of_code"`
}

// FileInfo holds per-file metadata used to construct Roads and provide git context.
type FileInfo struct {
	Path          string    `json:"path"`
	Extension     string    `json:"extension"`
	Depth         int       `json:"depth"`
	LinesOfCode   int       `json:"lines_of_code"`
	CommitChurn   int       `json:"commit_churn"`
	LastModified  time.Time `json:"last_modified"`
	PrimaryAuthor string    `json:"primary_author"`
	Includes      []string  `json:"includes"`
}

// RawExtraction is the full output of the AST parsing phase,
// before any LLM processing.
type RawExtraction struct {
	Files     []FileInfo    `json:"files"`
	Structs   []RawStruct   `json:"structs"`
	Functions []RawFunction `json:"functions"`
}

// Always ignored — these are never source code.
var ignoredDirs = map[string]bool{
	".git": true, "node_modules": true, "vendor": true,
	".github": true, "build": true, "dist": true, "target": true,
	"third_party": true, "thirdparty": true, "3rdparty": true,
	"external": true, "extern": true,
}

// isVendoredDir detects third-party directories by checking for
// their own LICENSE/README, or pre-compiled binary artifacts (.a, .so, .lib)
// which are a strong signal of vendored code.
func isVendoredDir(dirPath string) bool {
	// Check for LICENSE/README at this level
	indicators := []string{"LICENSE", "LICENSE.md", "LICENSE.txt", "README.md", "README"}
	for _, f := range indicators {
		if _, err := os.Stat(filepath.Join(dirPath, f)); err == nil {
			return true
		}
	}

	// Check for pre-compiled binaries (max 1 level deep)
	binaryExts := map[string]bool{".a": true, ".so": true, ".lib": true, ".dll": true, ".dylib": true}
	entries, err := os.ReadDir(dirPath)
	if err != nil {
		return false
	}
	for _, e := range entries {
		if !e.IsDir() {
			if binaryExts[filepath.Ext(e.Name())] {
				return true
			}
		} else {
			// Check one level of subdirectories (e.g., raylib/lib/libraylib.a)
			subEntries, err := os.ReadDir(filepath.Join(dirPath, e.Name()))
			if err != nil {
				continue
			}
			for _, se := range subEntries {
				if !se.IsDir() && binaryExts[filepath.Ext(se.Name())] {
					return true
				}
			}
		}
	}
	return false
}

// ExtractRepository walks the repo and extracts all raw structural data.
func ExtractRepository(clonePath string) (*RawExtraction, error) {
	result := &RawExtraction{}

	err := filepath.WalkDir(clonePath, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			if ignoredDirs[d.Name()] {
				fmt.Printf("[LOG]   Skipping ignored dir: %s\n", d.Name())
				return filepath.SkipDir
			}
			// Skip subdirectories that look like vendored third-party code
			if path != clonePath && isVendoredDir(path) {
				relDir, _ := filepath.Rel(clonePath, path)
				fmt.Printf("[LOG]   Skipping vendored dir: %s\n", relDir)
				return filepath.SkipDir
			}
			return nil
		}

		ext := filepath.Ext(d.Name())
		if ext != ".c" && ext != ".h" {
			return nil
		}

		fileInfo, structs, funcs, parseErr := parseCFile(path, clonePath)
		if parseErr != nil {
			fmt.Printf("[WARNING] Failed to parse %s: %v\n", path, parseErr)
			return nil
		}

		result.Files = append(result.Files, fileInfo)
		result.Structs = append(result.Structs, structs...)
		result.Functions = append(result.Functions, funcs...)
		return nil
	})

	if err != nil {
		return nil, fmt.Errorf("failed to walk directory: %w", err)
	}

	fmt.Printf("[LOG] Extracted %d files, %d structs, %d functions\n",
		len(result.Files), len(result.Structs), len(result.Functions))
	return result, nil
}

func parseCFile(fullPath, basePath string) (FileInfo, []RawStruct, []RawFunction, error) {
	content, err := os.ReadFile(fullPath)
	if err != nil {
		return FileInfo{}, nil, nil, err
	}

	parser := sitter.NewParser()
	parser.SetLanguage(c.GetLanguage())

	tree, err := parser.ParseCtx(context.Background(), nil, content)
	if err != nil {
		return FileInfo{}, nil, nil, err
	}

	rootNode := tree.RootNode()
	relPath, _ := filepath.Rel(basePath, fullPath)
	depth := strings.Count(relPath, string(filepath.Separator))

	fileInfo := FileInfo{
		Path:         relPath,
		Extension:    filepath.Ext(fullPath),
		Depth:        depth,
		LinesOfCode:  bytes.Count(content, []byte("\n")) + 1,
		CommitChurn:  getGitChurn(basePath, relPath),
		LastModified: getGitLastModified(basePath, relPath),
		PrimaryAuthor: getGitPrimaryAuthor(basePath, relPath),
	}

	var structs []RawStruct
	var funcs []RawFunction

	// Track nodes already processed via typedef to avoid double-counting
	seenNodes := make(map[uint32]bool)

	var walk func(*sitter.Node)
	walk = func(n *sitter.Node) {
		if seenNodes[n.StartByte()] {
			return
		}

		switch n.Type() {
		case "struct_specifier":
			s := extractStruct(n, content, relPath)
			if s != nil {
				structs = append(structs, *s)
			}

		case "type_definition":
			// A typedef wrapping a struct, e.g. typedef struct { ... } MyType;
			s := extractTypedefStruct(n, content, relPath)
			if s != nil {
				structs = append(structs, *s)
				// Mark inner struct_specifier as seen to prevent duplicate
				for i := 0; i < int(n.ChildCount()); i++ {
					child := n.Child(i)
					if child != nil && child.Type() == "struct_specifier" {
						seenNodes[child.StartByte()] = true
					}
				}
			}

		case "function_definition":
			f := extractFunction(n, content, relPath)
			if f != nil {
				funcs = append(funcs, *f)
			}

		case "preproc_include":
			if inc := nodeContent(n, content); inc != "" {
				fileInfo.Includes = append(fileInfo.Includes, inc)
			}
		}

		for i := 0; i < int(n.ChildCount()); i++ {
			if child := n.Child(i); child != nil {
				walk(child)
			}
		}
	}
	walk(rootNode)

	fmt.Printf("[LOG]   Parsed %s: %d structs, %d functions\n", relPath, len(structs), len(funcs))
	return fileInfo, structs, funcs, nil
}

// extractStruct pulls a named struct with its field list.
func extractStruct(node *sitter.Node, content []byte, filePath string) *RawStruct {
	name := ""
	var fields []string

	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child == nil {
			continue
		}
		switch child.Type() {
		case "type_identifier":
			name = nodeContent(child, content)
		case "field_declaration_list":
			fields = extractFieldNames(child, content)
		}
	}

	if name == "" {
		return nil
	}

	loc := int(node.EndPoint().Row-node.StartPoint().Row) + 1
	return &RawStruct{
		Name:        name,
		SourceFile:  filePath,
		Fields:      fields,
		LinesOfCode: loc,
		BodySnippet: minifySnippet(nodeContent(node, content), 500),
	}
}

// extractTypedefStruct handles: typedef struct { int x; } MyName;
func extractTypedefStruct(node *sitter.Node, content []byte, filePath string) *RawStruct {
	var innerStruct *sitter.Node
	typedefName := ""

	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child == nil {
			continue
		}
		switch child.Type() {
		case "struct_specifier":
			innerStruct = child
		case "type_identifier":
			typedefName = nodeContent(child, content)
		}
	}

	if innerStruct == nil {
		return nil // Not a struct typedef
	}

	// Try to get fields from the inner struct
	var fields []string
	for i := 0; i < int(innerStruct.ChildCount()); i++ {
		child := innerStruct.Child(i)
		if child != nil && child.Type() == "field_declaration_list" {
			fields = extractFieldNames(child, content)
		}
	}

	// Prefer typedef name, fall back to inner struct name
	name := typedefName
	if name == "" {
		for i := 0; i < int(innerStruct.ChildCount()); i++ {
			child := innerStruct.Child(i)
			if child != nil && child.Type() == "type_identifier" {
				name = nodeContent(child, content)
			}
		}
	}
	if name == "" {
		return nil
	}

	loc := int(node.EndPoint().Row-node.StartPoint().Row) + 1
	return &RawStruct{
		Name:        name,
		SourceFile:  filePath,
		Fields:      fields,
		LinesOfCode: loc,
		BodySnippet: minifySnippet(nodeContent(node, content), 500),
	}
}

func extractFieldNames(fieldList *sitter.Node, content []byte) []string {
	var fields []string
	for i := 0; i < int(fieldList.ChildCount()); i++ {
		child := fieldList.Child(i)
		if child == nil || child.Type() != "field_declaration" {
			continue
		}
		// The declarator usually contains the field name
		name := findDeclaratorName(child, content)
		if name != "" {
			fields = append(fields, name)
		}
	}
	return fields
}

func findDeclaratorName(node *sitter.Node, content []byte) string {
	// Look for field_identifier or identifier within declarators
	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child == nil {
			continue
		}
		switch child.Type() {
		case "field_identifier":
			return nodeContent(child, content)
		case "pointer_declarator", "array_declarator":
			if name := findDeclaratorName(child, content); name != "" {
				return name
			}
		}
	}
	return ""
}

func extractFunction(node *sitter.Node, content []byte, filePath string) *RawFunction {
	name := ""
	signature := ""

	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child == nil {
			continue
		}
		switch child.Type() {
		case "function_declarator":
			// Get function name from declarator
			for j := 0; j < int(child.ChildCount()); j++ {
				gc := child.Child(j)
				if gc == nil {
					continue
				}
				switch gc.Type() {
				case "identifier":
					name = nodeContent(gc, content)
				case "parameter_list":
					signature = nodeContent(gc, content)
				}
			}
		case "pointer_declarator":
			// Handle: int *myFunc(...)
			if n := findFuncDeclInPointer(child, content); n != "" {
				name = n
			}
		}
	}

	if name == "" {
		return nil
	}

	loc := int(node.EndPoint().Row-node.StartPoint().Row) + 1
	return &RawFunction{
		Name:        name,
		SourceFile:  filePath,
		Signature:   signature,
		LinesOfCode: loc,
		BodySnippet: minifySnippet(nodeContent(node, content), 600),
	}
}

func findFuncDeclInPointer(node *sitter.Node, content []byte) string {
	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child == nil {
			continue
		}
		if child.Type() == "function_declarator" {
			for j := 0; j < int(child.ChildCount()); j++ {
				gc := child.Child(j)
				if gc != nil && gc.Type() == "identifier" {
					return nodeContent(gc, content)
				}
			}
		}
		if child.Type() == "pointer_declarator" {
			if n := findFuncDeclInPointer(child, content); n != "" {
				return n
			}
		}
	}
	return ""
}

// ExtractSymbols extracts full source of named symbols for LLM context.
func ExtractSymbols(fullPath string, symbolNames []string) string {
	content, err := os.ReadFile(fullPath)
	if err != nil {
		return ""
	}

	parser := sitter.NewParser()
	parser.SetLanguage(c.GetLanguage())
	tree, err := parser.ParseCtx(context.Background(), nil, content)
	if err != nil {
		return ""
	}

	targets := make(map[string]bool)
	for _, f := range symbolNames {
		targets[strings.TrimSpace(f)] = true
	}

	var extracted bytes.Buffer
	extractTargetSymbols(tree.RootNode(), content, targets, &extracted)

	res := extracted.String()
	res = strings.ReplaceAll(res, "\t", " ")
	for strings.Contains(res, "  ") {
		res = strings.ReplaceAll(res, "  ", " ")
	}

	if len(res) > 1000 {
		return res[:400] + "\n...[truncated]...\n" + res[len(res)-400:]
	}
	return res
}

func extractTargetSymbols(node *sitter.Node, content []byte, targets map[string]bool, out *bytes.Buffer) {
	nodeType := node.Type()
	if nodeType == "function_definition" || nodeType == "struct_specifier" || nodeType == "type_definition" {
		name := findFirstIdentifier(node, content)
		if targets[name] {
			out.WriteString(nodeContent(node, content))
			out.WriteString("\n")
		}
	}
	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child != nil {
			extractTargetSymbols(child, content, targets, out)
		}
	}
}

func findFirstIdentifier(node *sitter.Node, content []byte) string {
	if node.Type() == "identifier" || node.Type() == "type_identifier" {
		return nodeContent(node, content)
	}
	for i := 0; i < int(node.ChildCount()); i++ {
		if res := findFirstIdentifier(node.Child(i), content); res != "" {
			return res
		}
	}
	return ""
}

func nodeContent(node *sitter.Node, content []byte) string {
	start := node.StartByte()
	end := node.EndByte()
	if start < uint32(len(content)) && end <= uint32(len(content)) && start <= end {
		return string(content[start:end])
	}
	return ""
}

func minifySnippet(s string, maxLen int) string {
	s = strings.ReplaceAll(s, "\t", " ")
	for strings.Contains(s, "  ") {
		s = strings.ReplaceAll(s, "  ", " ")
	}
	s = strings.ReplaceAll(s, "\n\n", "\n")
	if len(s) > maxLen {
		half := maxLen / 2
		return s[:half] + "\n...[truncated]...\n" + s[len(s)-half:]
	}
	return s
}

// ---- Git helpers ----

func getGitChurn(basePath, relPath string) int {
	cmd := exec.Command("git", "rev-list", "--count", "HEAD", "--", relPath)
	cmd.Dir = basePath
	out, err := cmd.Output()
	if err != nil {
		return 0
	}
	var count int
	fmt.Sscanf(strings.TrimSpace(string(out)), "%d", &count)
	return count
}

func getGitLastModified(basePath, relPath string) time.Time {
	cmd := exec.Command("git", "log", "-1", "--format=%cI", "--", relPath)
	cmd.Dir = basePath
	out, err := cmd.Output()
	if err != nil {
		return time.Now()
	}
	t, _ := time.Parse(time.RFC3339, strings.TrimSpace(string(out)))
	return t
}

func getGitPrimaryAuthor(basePath, relPath string) string {
	cmd := exec.Command("git", "log", "--format=%an", "--", relPath)
	cmd.Dir = basePath
	out, err := cmd.Output()
	if err != nil {
		return ""
	}

	authors := strings.Split(strings.TrimSpace(string(out)), "\n")
	counts := make(map[string]int)
	maxCount := 0
	primary := ""

	for _, a := range authors {
		a = strings.TrimSpace(a)
		if a == "" {
			continue
		}
		counts[a]++
		if counts[a] > maxCount {
			maxCount = counts[a]
			primary = a
		}
	}
	return primary
}
