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

	"github.com/repolis/repolis/backend/internal/models"
	sitter "github.com/smacker/go-tree-sitter"
	"github.com/smacker/go-tree-sitter/c"
)

func AnalyzeRepository(clonePath string) (*models.CityMap, error) {
	city := &models.CityMap{
		Files: []models.FileMetrics{},
	}

	ignoredDirs := map[string]bool{
		".git":         true,
		"node_modules": true,
		"vendor":       true,
		".github":      true,
		"build":        true,
	}

	err := filepath.WalkDir(clonePath, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		if d.IsDir() {
			if ignoredDirs[d.Name()] {
				return filepath.SkipDir
			}
			return nil
		}

		ext := filepath.Ext(d.Name())
		if ext != ".c" && ext != ".h" {
			return nil
		}

		metrics, parseErr := parseCFile(path, clonePath)
		if parseErr != nil {
			fmt.Printf("[WARN] Failed to parse %s: %v\n", path, parseErr)
			return nil
		}

		city.Files = append(city.Files, metrics)
		return nil
	})

	if err != nil {
		return nil, fmt.Errorf("failed to walk directory: %w", err)
	}

	return city, nil
}

func parseCFile(fullPath, basePath string) (models.FileMetrics, error) {
	content, err := os.ReadFile(fullPath)
	if err != nil {
		return models.FileMetrics{}, err
	}

	parser := sitter.NewParser()
	parser.SetLanguage(c.GetLanguage())

	tree, err := parser.ParseCtx(context.Background(), nil, content)
	if err != nil {
		return models.FileMetrics{}, err
	}

	rootNode := tree.RootNode()
	linesOfCode := bytes.Count(content, []byte("\n")) + 1
	relPath, _ := filepath.Rel(basePath, fullPath)
	depth := strings.Count(relPath, string(filepath.Separator))

	metrics := models.FileMetrics{
		Path:        relPath,
		Extension:   filepath.Ext(fullPath),
		Depth:       depth,
		LinesOfCode: linesOfCode,
	}

	extractASTData(rootNode, content, &metrics)

	metrics.CommitChurn = getGitChurn(basePath, relPath)
	metrics.LastModified = getGitLastModified(basePath, relPath)
	metrics.PrimaryAuthor = getGitPrimaryAuthor(basePath, relPath)

	return metrics, nil
}

func extractASTData(node *sitter.Node, content []byte, metrics *models.FileMetrics) {
	nodeType := node.Type()

	if nodeType == "function_definition" {
		metrics.FunctionCount++
		name := findFirstIdentifier(node, content)
		if name != "" {
			metrics.FunctionNames = append(metrics.FunctionNames, name)
		}
	} else if nodeType == "struct_specifier" || nodeType == "type_definition" {
		metrics.StructCount++
		name := findFirstIdentifier(node, content)
		if name != "" {
			metrics.StructNames = append(metrics.StructNames, name)
		}
	} else if nodeType == "preproc_include" {
		inc := extractNodeContent(node, content)
		if inc != "" {
			metrics.Includes = append(metrics.Includes, inc)
		}
	} else if nodeType == "string_literal" {
		str := extractNodeContent(node, content)
		if str != "" {
			metrics.StringLiterals = append(metrics.StringLiterals, str)
		}
	} else if nodeType == "comment" {
		comment := strings.ToUpper(extractNodeContent(node, content))
		if strings.Contains(comment, "TODO") || strings.Contains(comment, "FIXME") || strings.Contains(comment, "HACK") {
			metrics.TodoCount++
		}
	}

	for i := 0; i < int(node.ChildCount()); i++ {
		child := node.Child(i)
		if child != nil {
			extractASTData(child, content, metrics)
		}
	}
}

func findFirstIdentifier(node *sitter.Node, content []byte) string {
	if node.Type() == "identifier" || node.Type() == "type_identifier" {
		return extractNodeContent(node, content)
	}
	for i := 0; i < int(node.ChildCount()); i++ {
		if res := findFirstIdentifier(node.Child(i), content); res != "" {
			return res
		}
	}
	return ""
}

func extractNodeContent(node *sitter.Node, content []byte) string {
	start := node.StartByte()
	end := node.EndByte()
	if start < uint32(len(content)) && end <= uint32(len(content)) && start <= end {
		return string(content[start:end])
	}
	return ""
}

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
