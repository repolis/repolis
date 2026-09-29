package analyzer

import (
	"bytes"
	"context"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"

	"github.com/repolis/repolis/backend/internal/logger"

	"github.com/repolis/repolis/backend/internal/analyzer/lang"
	sitter "github.com/smacker/go-tree-sitter"
)

// Directories that are never first-party source.
var ignoredDirs = map[string]bool{
	".git": true, "node_modules": true, "vendor": true, ".github": true,
	"build": true, "dist": true, "target": true, "cmake-build-debug": true,
	"third_party": true, "thirdparty": true, "3rdparty": true,
	"external": true, "extern": true, "deps": true, "depends": true,
	"subprojects": true, "contrib": true, ".deps": true, "m4": true,
}

var licenseNames = map[string]bool{
	"LICENSE": true, "LICENSE.md": true, "LICENSE.txt": true,
	"LICENCE": true, "LICENCE.md": true, "LICENCE.txt": true,
	"COPYING": true, "COPYING.txt": true, "COPYRIGHT": true,
}

// firstPartyDirs are never vendored, whatever they contain.
var firstPartyDirs = map[string]bool{
	"src": true, "lib": true, "include": true, "examples": true, "example": true,
	"test": true, "tests": true, "demo": true, "demos": true, "tools": true,
	"tool": true, "cmd": true, "app": true, "apps": true, "samples": true,
	"crates": true, "packages": true, "internal": true, "pkg": true,
}

var binaryExts = map[string]bool{
	".a": true, ".so": true, ".lib": true, ".dll": true, ".dylib": true, ".o": true,
}

// isVendoredDir detects a third-party tree checked into the repository. The
// only signal left is a licence file next to a pre-compiled binary; every
// cheaper one had false positives (a README cost libgit2 its examples, a
// nested manifest cost ripgrep three of its own workspace crates). Directory
// names that really do mean vendoring are handled by ignoredDirs above.
func isVendoredDir(dirPath string) bool {
	if firstPartyDirs[strings.ToLower(filepath.Base(dirPath))] {
		return false
	}
	entries, err := os.ReadDir(dirPath)
	if err != nil {
		return false
	}

	hasLicense, hasBinary := false, false
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := e.Name()
		if licenseNames[strings.ToUpper(name)] {
			hasLicense = true
		}
		if binaryExts[filepath.Ext(name)] {
			hasBinary = true
		}
	}
	return hasLicense && hasBinary
}

type parseJob struct {
	fullPath string
	relPath  string
	lang     lang.Language
}

type parseOutput struct {
	file    FileInfo
	structs []RawStruct
	funcs   []RawFunction
}

// ExtractRepository walks the repo and extracts everything in parallel:
// parsing is pure CPU work with no shared state, so it scales with cores.
func ExtractRepository(clonePath string) (*RawExtraction, error) {
	var jobs []parseJob
	var skipped []string

	err := filepath.WalkDir(clonePath, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			if path != clonePath && ignoredDirs[d.Name()] {
				rel, _ := filepath.Rel(clonePath, path)
				skipped = append(skipped, rel)
				return filepath.SkipDir
			}
			if path != clonePath && isVendoredDir(path) {
				rel, _ := filepath.Rel(clonePath, path)
				logger.Log(logger.WarnLevel, "Skipping vendored dir (licence + build/binary): %s", rel)
				skipped = append(skipped, rel)
				return filepath.SkipDir
			}
			return nil
		}

		l := lang.ForFile(d.Name())
		if l == nil {
			return nil
		}
		rel, _ := filepath.Rel(clonePath, path)
		jobs = append(jobs, parseJob{
			fullPath: path,
			relPath:  filepath.ToSlash(rel),
			lang:     l,
		})
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("failed to walk directory: %w", err)
	}

	sort.Slice(jobs, func(i, j int) bool { return jobs[i].relPath < jobs[j].relPath })

	workers := runtime.NumCPU()
	if workers > 8 {
		workers = 8
	}
	if workers < 1 {
		workers = 1
	}

	outputs := make([]parseOutput, len(jobs))
	var wg sync.WaitGroup
	jobCh := make(chan int, len(jobs))
	for i := range jobs {
		jobCh <- i
	}
	close(jobCh)

	for w := 0; w < workers; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			// Per worker: tree-sitter parsers are not goroutine safe, and one
			// per file is wasteful.
			parsers := make(map[string]*sitter.Parser)
			for idx := range jobCh {
				j := jobs[idx]
				p, ok := parsers[j.lang.Name()]
				if !ok {
					p = sitter.NewParser()
					p.SetLanguage(j.lang.Grammar())
					parsers[j.lang.Name()] = p
				}
				out, perr := parseFile(p, j.lang, j.fullPath, j.relPath)
				if perr != nil {
					logger.Log(logger.WarnLevel, "Failed to parse %s: %v", j.relPath, perr)
					continue
				}
				outputs[idx] = out
			}
		}()
	}
	wg.Wait()

	result := &RawExtraction{SkippedDirs: skipped, Languages: map[string]int{}}
	for _, out := range outputs {
		if out.file.Path == "" {
			continue
		}
		result.Files = append(result.Files, out.file)
		result.Structs = append(result.Structs, out.structs...)
		result.Functions = append(result.Functions, out.funcs...)
		result.Languages[out.file.Language]++
	}

	logger.Log(logger.InfoLevel,
		"Extracted %d files, %d type definitions, %d functions (%d workers, %d dirs skipped, languages: %v)",
		len(result.Files), len(result.Structs), len(result.Functions), workers, len(skipped), result.Languages)
	return result, nil
}

// parseFile runs one file through whichever language claims it.
func parseFile(parser *sitter.Parser, l lang.Language, fullPath, relPath string) (parseOutput, error) {
	content, err := os.ReadFile(fullPath)
	if err != nil {
		return parseOutput{}, err
	}
	// Generated blobs (amalgamations, tables) cost parse time for nothing.
	if len(content) > 4*1024*1024 {
		return parseOutput{}, fmt.Errorf("file too large (%d bytes)", len(content))
	}

	tree, err := parser.ParseCtx(context.Background(), nil, content)
	if err != nil {
		return parseOutput{}, err
	}
	defer tree.Close()

	dir := filepath.ToSlash(filepath.Dir(relPath))
	if dir == "." || dir == "/" {
		dir = "root"
	}
	ns := l.Namespace(relPath)

	facts := l.Parse(tree.RootNode(), content)

	out := parseOutput{
		file: FileInfo{
			Path:          relPath,
			Dir:           dir,
			Extension:     filepath.Ext(relPath),
			Language:      l.Name(),
			Namespace:     ns,
			Depth:         strings.Count(relPath, "/"),
			LinesOfCode:   bytes.Count(content, []byte("\n")) + 1,
			FileScopeVars: facts.ScopeVar,
			Imports:       facts.Imports,
		},
	}

	for _, t := range facts.Types {
		out.structs = append(out.structs, RawStruct{
			Name: t.Name, Namespace: ns, SourceFile: relPath,
			Fields: t.Fields, FieldTypes: t.FieldTypes, LinesOfCode: t.LOC,
		})
	}
	for _, f := range facts.Funcs {
		out.funcs = append(out.funcs, RawFunction{
			Name: f.Name, Namespace: ns, Receiver: f.Receiver, SourceFile: relPath,
			Signature: f.Signature, ReturnType: f.ReturnType, ParamTypes: f.ParamTypes,
			TypesUsed: f.TypesUsed, Calls: f.Calls, Complexity: f.Complexity,
			LinesOfCode: f.LOC,
		})
	}
	return out, nil
}

func nodeContent(node *sitter.Node, content []byte) string {
	if node == nil {
		return ""
	}
	start, end := node.StartByte(), node.EndByte()
	if start <= end && end <= uint32(len(content)) {
		return string(content[start:end])
	}
	return ""
}

func collapseSpaces(s string) string {
	return strings.Join(strings.Fields(strings.ReplaceAll(s, "\n", " ")), " ")
}
