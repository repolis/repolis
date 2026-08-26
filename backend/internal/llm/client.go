package llm

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"sync"

	"github.com/repolis/repolis/backend/internal/analyzer"
	"github.com/repolis/repolis/backend/internal/models"
	openai "github.com/sashabaranov/go-openai"
)

type Client struct {
	api   *openai.Client
	model string
}

func NewClient() (*Client, error) {
	baseURL := os.Getenv("LLM_BASE_URL")
	if baseURL == "" {
		return nil, fmt.Errorf("LLM_BASE_URL is not set")
	}

	model := os.Getenv("LLM_MODEL")
	if model == "" {
		return nil, fmt.Errorf("LLM_MODEL is not set")
	}

	apiKey := os.Getenv("LLM_API_KEY")
	if apiKey == "" {
		apiKey = "ollama"
	}

	config := openai.DefaultConfig(apiKey)
	config.BaseURL = baseURL

	return &Client{
		api:   openai.NewClientWithConfig(config),
		model: model,
	}, nil
}

// BuildCity takes raw AST data and uses the LLM to produce a full CityMap.
// Pipeline:
//   1. Associate functions → structs (method inference)
//   2. Generate building summaries
//   3. Group buildings into semantic districts
//   4. Build roads from filesystem paths
func (c *Client) BuildCity(ctx context.Context, clonePath string, raw *analyzer.RawExtraction) (*models.CityMap, error) {
	fmt.Println("[LOG] Phase 1: LLM method association...")
	buildings, orphanFuncs := c.associateMethods(ctx, clonePath, raw)

	fmt.Printf("[LOG]   → %d buildings, %d orphan functions\n", len(buildings), len(orphanFuncs))

	// Promote significant orphan function clusters into standalone buildings
	promoted := promoteOrphanFunctions(orphanFuncs, raw)
	buildings = append(buildings, promoted...)
	fmt.Printf("[LOG]   → %d total buildings after promotion\n", len(buildings))

	fmt.Println("[LOG] Phase 2: LLM building summaries...")
	c.summarizeBuildings(ctx, clonePath, buildings)

	fmt.Println("[LOG] Phase 3: LLM semantic district grouping...")
	districts := c.groupIntoDistricts(ctx, buildings)

	fmt.Println("[LOG] Phase 4: Building roads from filesystem...")
	roads := buildRoads(raw, buildings)

	fmt.Println("[LOG] Phase 5: Extracting include-graph dependencies...")
	dependencies := buildDependencies(raw, buildings)
	fmt.Printf("[LOG]   → %d dependency edges\n", len(dependencies))

	city := &models.CityMap{
		Districts:    districts,
		Roads:        roads,
		Dependencies: dependencies,
	}

	fmt.Printf("[LOG] City complete: %d districts, %d roads, %d dependencies\n", len(districts), len(roads), len(dependencies))
	return city, nil
}

// Phase 1: Associate functions → structs via LLM

type methodAssociation struct {
	StructName string   `json:"struct_name"`
	Methods    []string `json:"methods"`
}

func (c *Client) associateMethods(ctx context.Context, clonePath string, raw *analyzer.RawExtraction) ([]models.Building, []analyzer.RawFunction) {
	// Build buildings from structs with guaranteed non-nil slices
	buildingsByName := make(map[string]*models.Building)
	for _, s := range raw.Structs {
		fields := s.Fields
		if fields == nil {
			fields = make([]string, 0)
		}
		b := &models.Building{
			Name:        s.Name,
			SourceFile:  s.SourceFile,
			NumFields:   len(fields),
			Fields:      fields,
			LinesOfCode: s.LinesOfCode,
			Methods:     make([]string, 0),
		}
		buildingsByName[s.Name] = b
	}

	if len(raw.Structs) == 0 {
		fmt.Println("[LOG]   No structs in project; skipping LLM association.")
		return make([]models.Building, 0), raw.Functions
	}

	// Group functions and structs by source file
	funcsByFile := make(map[string][]analyzer.RawFunction)
	for _, f := range raw.Functions {
		funcsByFile[f.SourceFile] = append(funcsByFile[f.SourceFile], f)
	}
	structsByFile := make(map[string][]analyzer.RawStruct)
	for _, s := range raw.Structs {
		structsByFile[s.SourceFile] = append(structsByFile[s.SourceFile], s)
	}

	allStructNames := make([]string, 0, len(raw.Structs))
	for _, s := range raw.Structs {
		allStructNames = append(allStructNames, s.Name)
	}

	type assocResult struct {
		filePath     string
		associations []methodAssociation
		orphans      []analyzer.RawFunction
	}
	resChan := make(chan assocResult, len(funcsByFile))
	sem := make(chan struct{}, 4) // Worker pool of 4 concurrent workers
	var wg sync.WaitGroup

	fileIdx := 0
	totalFiles := len(funcsByFile)
	for filePath, fileFuncs := range funcsByFile {
		fileIdx++
		if len(fileFuncs) == 0 {
			continue
		}

		wg.Add(1)
		go func(idx int, path string, funcs []analyzer.RawFunction) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			fmt.Printf("[LOG]   [%d/%d] Associating %d functions in %s (concurrent)...\n", idx, totalFiles, len(funcs), path)

			funcSummaries := make([]string, 0, len(funcs))
			for _, f := range funcs {
				funcSummaries = append(funcSummaries, fmt.Sprintf("%s%s", f.Name, f.Signature))
			}
			localStructs := structsByFile[path]
			localStructSummaries := make([]string, 0, len(localStructs))
			for _, s := range localStructs {
				localStructSummaries = append(localStructSummaries, fmt.Sprintf("%s{%s}", s.Name, strings.Join(s.Fields, ", ")))
			}

			symbolNames := make([]string, 0)
			for _, f := range funcs {
				symbolNames = append(symbolNames, f.Name)
			}
			codeContext := ""
			if len(symbolNames) > 0 {
				fullPath := filepath.Join(clonePath, path)
				codeContext = analyzer.ExtractSymbols(fullPath, symbolNames)
			}

			prompt := fmt.Sprintf(
				`Given this C source file, associate each function with the struct it primarily operates on.

File: %s

Structs in this file: %s
All known structs in project: %s

Functions in this file:
%s

Code context:
%s

Output a JSON array. Each element: {"struct_name": "StructName", "methods": ["func1", "func2"]}.
If a function doesn't belong to any struct, associate it with "NONE".
Output ONLY raw JSON array, no markdown, no explanation.`,
				path,
				strings.Join(localStructSummaries, "; "),
				strings.Join(allStructNames, ", "),
				strings.Join(funcSummaries, "\n"),
				codeContext,
			)

			resp, err := c.api.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
				Model: c.model,
				Messages: []openai.ChatCompletionMessage{
					{Role: openai.ChatMessageRoleUser, Content: prompt},
				},
				Temperature: 0.0,
			})

			if err != nil || len(resp.Choices) == 0 {
				resChan <- assocResult{filePath: path, orphans: funcs}
				return
			}

			content := cleanJSON(resp.Choices[0].Message.Content)
			var associations []methodAssociation
			if err := json.Unmarshal([]byte(content), &associations); err != nil {
				resChan <- assocResult{filePath: path, orphans: funcs}
				return
			}
			resChan <- assocResult{filePath: path, associations: associations, orphans: funcs}
		}(fileIdx, filePath, fileFuncs)
	}

	wg.Wait()
	close(resChan)

	var orphanFunctions []analyzer.RawFunction
	for r := range resChan {
		assignedFuncs := make(map[string]bool)
		for _, assoc := range r.associations {
			if assoc.StructName == "NONE" || assoc.StructName == "" {
				continue
			}
			if b, ok := buildingsByName[assoc.StructName]; ok {
				for _, m := range assoc.Methods {
					b.Methods = append(b.Methods, m)
					b.NumMethods++
					assignedFuncs[m] = true
				}
			}
		}
		for _, f := range r.orphans {
			if !assignedFuncs[f.Name] {
				orphanFunctions = append(orphanFunctions, f)
			}
		}
	}

	buildings := make([]models.Building, 0, len(buildingsByName))
	for _, b := range buildingsByName {
		buildings = append(buildings, *b)
	}
	sort.Slice(buildings, func(i, j int) bool {
		return buildings[i].Name < buildings[j].Name
	})

	return buildings, orphanFunctions
}

// promoteOrphanFunctions creates "utility" buildings from files that have many functions but no structs.
func promoteOrphanFunctions(orphans []analyzer.RawFunction, raw *analyzer.RawExtraction) []models.Building {
	byFile := make(map[string][]analyzer.RawFunction)
	for _, f := range orphans {
		byFile[f.SourceFile] = append(byFile[f.SourceFile], f)
	}

	fileInfoMap := make(map[string]analyzer.FileInfo)
	for _, fi := range raw.Files {
		fileInfoMap[fi.Path] = fi
	}

	buildings := make([]models.Building, 0)
	for filePath, funcs := range byFile {
		if len(funcs) < 2 {
			continue
		}

		baseName := strings.TrimSuffix(filepath.Base(filePath), filepath.Ext(filePath))
		methods := make([]string, 0, len(funcs))
		totalLOC := 0
		for _, f := range funcs {
			methods = append(methods, f.Name)
			totalLOC += f.LinesOfCode
		}

		fi := fileInfoMap[filePath]
		buildings = append(buildings, models.Building{
			Name:         baseName,
			SourceFile:   filePath,
			NumFields:    0,
			NumMethods:   len(methods),
			Fields:       make([]string, 0),
			Methods:      methods,
			LinesOfCode:  totalLOC,
			CommitChurn:  fi.CommitChurn,
			LastModified: fi.LastModified,
		})
	}

	sort.Slice(buildings, func(i, j int) bool {
		return buildings[i].Name < buildings[j].Name
	})

	return buildings
}

// Phase 2: Generate building summaries

func (c *Client) summarizeBuildings(ctx context.Context, clonePath string, buildings []models.Building) {
	var needsLLM []*models.Building
	zeroCostCount := 0

	for i := range buildings {
		b := &buildings[i]
		if b.Fields == nil {
			b.Fields = make([]string, 0)
		}
		if b.Methods == nil {
			b.Methods = make([]string, 0)
		}
		// Zero-cost summary for promoted modules / utility files or symbols with no complex fields
		if b.NumFields == 0 {
			b.Summary = fmt.Sprintf("Utility module with %d functions in %s", b.NumMethods, filepath.Base(b.SourceFile))
			zeroCostCount++
			continue
		}
		needsLLM = append(needsLLM, b)
	}

	fmt.Printf("[LOG]   Applied %d zero-cost summaries; %d structs require AI abstraction\n", zeroCostCount, len(needsLLM))
	if len(needsLLM) == 0 {
		return
	}

	batchSize := 15
	var batches [][]*models.Building
	for i := 0; i < len(needsLLM); i += batchSize {
		end := i + batchSize
		if end > len(needsLLM) {
			end = len(needsLLM)
		}
		batches = append(batches, needsLLM[i:end])
	}

	fmt.Printf("[LOG]   Processing %d batches concurrently via worker pool...\n", len(batches))
	sem := make(chan struct{}, 4) // max 4 concurrent AI summarization workers
	var wg sync.WaitGroup

	for bIdx, batch := range batches {
		wg.Add(1)
		go func(batchNum int, bSlice []*models.Building) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			var manifest strings.Builder
			for idx, b := range bSlice {
				manifest.WriteString(fmt.Sprintf("[ID: %d] %s (File: %s, Fields: [%s], Methods: [%s])\n",
					idx, b.Name, filepath.Base(b.SourceFile),
					strings.Join(b.Fields, ","),
					strings.Join(b.Methods, ",")))
			}

			prompt := fmt.Sprintf(`Summarize each C code entity below in 5-10 words per entity.
%s
Output ONLY a raw JSON array of objects with keys "id" (integer) and "summary" (string). No markdown, no comments, no explanation.`, manifest.String())

			resp, err := c.api.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
				Model: c.model,
				Messages: []openai.ChatCompletionMessage{
					{Role: openai.ChatMessageRoleUser, Content: prompt},
				},
				Temperature: 0.0,
			})

			type sumResp struct {
				ID      int    `json:"id"`
				Summary string `json:"summary"`
			}
			success := false
			if err == nil && len(resp.Choices) > 0 {
				content := cleanJSON(resp.Choices[0].Message.Content)
				var results []sumResp
				if json.Unmarshal([]byte(content), &results) == nil {
					sumMap := make(map[int]string)
					for _, r := range results {
						sumMap[r.ID] = r.Summary
					}
					for idx, b := range bSlice {
						if s, ok := sumMap[idx]; ok && s != "" {
							b.Summary = strings.TrimSpace(s)
						} else {
							b.Summary = fmt.Sprintf("Data struct %s in %s", b.Name, filepath.Base(b.SourceFile))
						}
					}
					success = true
				}
			}
			if !success {
				// Zero-cost fallback summary if LLM batch failed
				for _, b := range bSlice {
					b.Summary = fmt.Sprintf("Data struct %s with %d fields in %s", b.Name, b.NumFields, filepath.Base(b.SourceFile))
				}
			}
			fmt.Printf("[LOG]     Completed AI summary batch %d/%d\n", batchNum+1, len(batches))
		}(bIdx, batch)
	}
	wg.Wait()
}

// Phase 3: Group buildings into semantic districts via LLM

type districtAssignment struct {
	DistrictName string   `json:"district_name"`
	Typology     string   `json:"typology"`
	Summary      string   `json:"summary"`
	Tags         []string `json:"tags"`
	BuildingIDs  []int    `json:"building_ids"`
}

func (c *Client) groupIntoDistricts(ctx context.Context, buildings []models.Building) []models.District {
	if len(buildings) == 0 {
		return make([]models.District, 0)
	}

	if len(buildings) <= 35 {
		return c.groupIntoDistrictsChunk(ctx, buildings, 0)
	}

	fmt.Printf("[LOG]   Large codebase detected (%d buildings). Executing Hierarchical MapReduce Clustering...\n", len(buildings))

	byDir := make(map[string][]models.Building)
	for _, b := range buildings {
		dir := filepath.Dir(b.SourceFile)
		if dir == "." || dir == "/" {
			dir = "root"
		}
		byDir[dir] = append(byDir[dir], b)
	}

	var dirs []string
	for d := range byDir {
		dirs = append(dirs, d)
	}
	sort.Strings(dirs)

	var chunks [][]models.Building
	var currentChunk []models.Building

	for _, d := range dirs {
		bs := byDir[d]
		for len(bs) > 0 {
			remaining := 35 - len(currentChunk)
			if remaining <= 0 {
				chunks = append(chunks, currentChunk)
				currentChunk = nil
				remaining = 35
			}
			take := len(bs)
			if take > remaining {
				take = remaining
			}
			currentChunk = append(currentChunk, bs[:take]...)
			bs = bs[take:]
		}
	}
	if len(currentChunk) > 0 {
		chunks = append(chunks, currentChunk)
	}

	fmt.Printf("[LOG]     Map Stage: Divided %d buildings into %d localized clusters; processing concurrently...\n", len(buildings), len(chunks))

	sem := make(chan struct{}, 4)
	var wg sync.WaitGroup
	chunkDistricts := make([][]models.District, len(chunks))

	for idx, chunk := range chunks {
		wg.Add(1)
		go func(chunkIdx int, bSlice []models.Building) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			chunkDistricts[chunkIdx] = c.groupIntoDistrictsChunk(ctx, bSlice, chunkIdx+1)
			fmt.Printf("[LOG]     Completed district cluster %d/%d\n", chunkIdx+1, len(chunks))
		}(idx, chunk)
	}
	wg.Wait()

	var allLocal []models.District
	for _, ds := range chunkDistricts {
		allLocal = append(allLocal, ds...)
	}

	fmt.Printf("[LOG]     Reduce Stage: Consolidating %d localized districts...\n", len(allLocal))
	return c.reduceDistricts(allLocal)
}

func (c *Client) groupIntoDistrictsChunk(ctx context.Context, buildings []models.Building, chunkNum int) []models.District {
	if len(buildings) == 0 {
		return make([]models.District, 0)
	}

	var manifest strings.Builder
	for i, b := range buildings {
		manifest.WriteString(fmt.Sprintf(
			"- [ID: %d] %s (file: %s, fields: %d, methods: %d, summary: %s)\n",
			i, b.Name, filepath.Base(b.SourceFile),
			b.NumFields, b.NumMethods, b.Summary,
		))
	}

	systemPrompt := `You are an expert software architect analyzing a C codebase.
Your task: group code entities into logical SEMANTIC districts — like neighborhoods in a city.

Districts should reflect LOGICAL relationships: entities that work together belong in the same district.

Each district MUST have:
- "district_name": descriptive 2-4 word name
- "typology": EXACTLY ONE of: core, data, network, security, interface, utility, config, test, example
- "summary": 5-10 word description of the district's purpose
- "tags": 2-4 short technical keywords
- "building_ids": array of integer IDs of the buildings that belong in this district

RULES:
1. Every building ID must appear in exactly ONE district.
2. Create 2 to 6 districts.
3. If buildings have no clear relationship, group them in a "Utilities & Helpers" district.
4. Output ONLY a raw JSON array of district objects. No markdown, no explanation.
5. Produce strictly valid, parseable JSON.`

	prompt := fmt.Sprintf("Here are the code entities to group:\n\n%s\n\nGroup them into semantic districts.", manifest.String())

	resp, err := c.api.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
		Model: c.model,
		Messages: []openai.ChatCompletionMessage{
			{Role: openai.ChatMessageRoleSystem, Content: systemPrompt},
			{Role: openai.ChatMessageRoleUser, Content: prompt},
		},
		Temperature: 0.0,
	})

	if err != nil || len(resp.Choices) == 0 {
		if chunkNum > 0 {
			fmt.Printf("[WARNING] LLM district grouping failed for cluster %d: %v\n", chunkNum, err)
		} else {
			fmt.Printf("[WARNING] LLM district grouping failed: %v\n", err)
		}
		return fallbackDistricts(buildings)
	}

	content := cleanJSON(resp.Choices[0].Message.Content)
	var assignments []districtAssignment
	if err := json.Unmarshal([]byte(content), &assignments); err != nil {
		if chunkNum > 0 {
			fmt.Printf("[WARNING] Failed to parse district assignments for cluster %d: %v\n", chunkNum, err)
		} else {
			fmt.Printf("[WARNING] Failed to parse district assignments: %v\n", err)
		}
		return fallbackDistricts(buildings)
	}

	assigned := make(map[int]bool)
	districts := make([]models.District, 0, len(assignments)+1)

	for _, da := range assignments {
		tags := da.Tags
		if tags == nil {
			tags = make([]string, 0)
		}
		d := models.District{
			Name:      da.DistrictName,
			Typology:  validateTypology(da.Typology),
			Summary:   da.Summary,
			Tags:      tags,
			Buildings: make([]models.Building, 0),
		}

		for _, bID := range da.BuildingIDs {
			if bID >= 0 && bID < len(buildings) && !assigned[bID] {
				d.Buildings = append(d.Buildings, buildings[bID])
				assigned[bID] = true
			}
		}

		if len(d.Buildings) > 0 {
			districts = append(districts, d)
		}
	}

	unassigned := make([]models.Building, 0)
	for i, b := range buildings {
		if !assigned[i] {
			unassigned = append(unassigned, b)
		}
	}
	if len(unassigned) > 0 {
		districts = append(districts, models.District{
			Name:      "Miscellaneous",
			Typology:  "utility",
			Summary:   "Ungrouped code entities",
			Tags:      []string{"misc", "ungrouped"},
			Buildings: unassigned,
		})
	}

	sort.Slice(districts, func(i, j int) bool {
		return districts[i].Name < districts[j].Name
	})

	return districts
}

func (c *Client) reduceDistricts(localDistricts []models.District) []models.District {
	// 1. Merge identically named districts (case-insensitive)
	mergedByName := make(map[string]*models.District)
	for _, d := range localDistricts {
		key := strings.ToLower(strings.TrimSpace(d.Name))
		if existing, found := mergedByName[key]; found {
			existing.Buildings = append(existing.Buildings, d.Buildings...)
			for _, tag := range d.Tags {
				existing.Tags = append(existing.Tags, tag)
			}
		} else {
			copyD := d
			if copyD.Tags == nil {
				copyD.Tags = make([]string, 0)
			}
			if copyD.Buildings == nil {
				copyD.Buildings = make([]models.Building, 0)
			}
			mergedByName[key] = &copyD
		}
	}

	var candidates []models.District
	for _, d := range mergedByName {
		candidates = append(candidates, *d)
	}

	// 2. If > 20 districts, sort by size and consolidate smaller ones by typology
	if len(candidates) > 20 {
		sort.Slice(candidates, func(i, j int) bool {
			return len(candidates[i].Buildings) > len(candidates[j].Buildings)
		})

		keepers := candidates[:14]
		smaller := candidates[14:]

		byTypo := make(map[string]*models.District)
		for _, d := range smaller {
			t := d.Typology
			if byTypo[t] == nil {
				titleTypo := t
				if len(t) > 0 {
					titleTypo = strings.ToUpper(t[:1]) + t[1:]
				}
				byTypo[t] = &models.District{
					Name:      fmt.Sprintf("Consolidated %s Architecture", titleTypo),
					Typology:  t,
					Summary:   fmt.Sprintf("Unified collection of %s subsystems", t),
					Tags:      []string{t, "consolidated"},
					Buildings: make([]models.Building, 0),
				}
			}
			byTypo[t].Buildings = append(byTypo[t].Buildings, d.Buildings...)
		}

		candidates = keepers
		for _, d := range byTypo {
			if len(d.Buildings) > 0 {
				candidates = append(candidates, *d)
			}
		}
	}

	// Ensure non-nil slices and sort
	for i := range candidates {
		if candidates[i].Tags == nil {
			candidates[i].Tags = make([]string, 0)
		}
		if candidates[i].Buildings == nil {
			candidates[i].Buildings = make([]models.Building, 0)
		}
	}
	sort.Slice(candidates, func(i, j int) bool {
		return candidates[i].Name < candidates[j].Name
	})

	return candidates
}

func fallbackDistricts(buildings []models.Building) []models.District {
	byDir := make(map[string][]models.Building)
	for _, b := range buildings {
		dir := filepath.Dir(b.SourceFile)
		if dir == "." || dir == "/" {
			dir = "root"
		}
		byDir[dir] = append(byDir[dir], b)
	}

	districts := make([]models.District, 0, len(byDir))
	for dir, bs := range byDir {
		for i := range bs {
			if bs[i].Fields == nil {
				bs[i].Fields = make([]string, 0)
			}
			if bs[i].Methods == nil {
				bs[i].Methods = make([]string, 0)
			}
		}
		districts = append(districts, models.District{
			Name:      dir,
			Typology:  "unknown",
			Summary:   fmt.Sprintf("Code from %s", dir),
			Tags:      make([]string, 0),
			Buildings: bs,
		})
	}

	sort.Slice(districts, func(i, j int) bool {
		return districts[i].Name < districts[j].Name
	})

	return districts
}

// Phase 4: Build roads from filesystem paths

func buildRoads(raw *analyzer.RawExtraction, buildings []models.Building) []models.Road {
	dirBuildings := make(map[string][]string)
	dirDepth := make(map[string]int)
	dirFileCount := make(map[string]int)

	for _, b := range buildings {
		dir := filepath.Dir(b.SourceFile)
		if dir == "." || dir == "/" {
			dir = "root"
		}
		dirBuildings[dir] = append(dirBuildings[dir], b.Name)
	}

	for _, f := range raw.Files {
		dir := filepath.Dir(f.Path)
		if dir == "." || dir == "/" {
			dir = "root"
		}
		dirFileCount[dir]++
		dirDepth[dir] = f.Depth
	}

	roads := make([]models.Road, 0, len(dirFileCount))
	for dir, bNames := range dirBuildings {
		if bNames == nil {
			bNames = make([]string, 0)
		}
		roads = append(roads, models.Road{
			Name:      filepath.Base(dir),
			FullPath:  dir,
			Depth:     dirDepth[dir],
			FileCount: dirFileCount[dir],
			Buildings: bNames,
		})
	}

	for dir := range dirFileCount {
		if _, exists := dirBuildings[dir]; !exists {
			roads = append(roads, models.Road{
				Name:      filepath.Base(dir),
				FullPath:  dir,
				Depth:     dirDepth[dir],
				FileCount: dirFileCount[dir],
				Buildings: make([]string, 0),
			})
		}
	}

	return roads
}

// Phase 5: Build dependency edges from #include graph
//
// Resolves file-level #include relationships into building-to-building
// edges. Zero LLM cost — purely derived from tree-sitter's include
// extraction. For large repos, edges are capped and sorted by weight
// so only significant connections are rendered as roads.

func buildDependencies(raw *analyzer.RawExtraction, buildings []models.Building) []models.DependencyEdge {
	// 1. Map each file path to the buildings it contains
	fileToBuildingNames := make(map[string][]string)
	for _, b := range buildings {
		if b.SourceFile != "" {
			fileToBuildingNames[b.SourceFile] = append(fileToBuildingNames[b.SourceFile], b.Name)
		}
	}

	// 2. Build a lookup from filename (basename) and relative path to full rel path
	//    This handles includes like "raylib.h" matching "src/raylib.h"
	basenameToFiles := make(map[string][]string)
	for _, fi := range raw.Files {
		base := filepath.Base(fi.Path)
		basenameToFiles[base] = append(basenameToFiles[base], fi.Path)
	}

	// 3. Resolve each file's includes into building-to-building edges
	type edgeKey struct{ src, tgt string }
	edgeCounts := make(map[edgeKey]int)

	for _, fi := range raw.Files {
		srcBuildings := fileToBuildingNames[fi.Path]
		if len(srcBuildings) == 0 {
			continue
		}

		for _, inc := range fi.Includes {
			// Extract the path from #include "..." or #include <...>
			incPath := extractIncludePath(inc)
			if incPath == "" {
				continue
			}

			// Resolve include to known files
			targetFiles := resolveInclude(incPath, fi.Path, basenameToFiles)
			for _, tgtFile := range targetFiles {
				if tgtFile == fi.Path {
					continue // skip self-includes
				}
				tgtBuildings := fileToBuildingNames[tgtFile]
				for _, src := range srcBuildings {
					for _, tgt := range tgtBuildings {
						if src != tgt {
							edgeCounts[edgeKey{src, tgt}]++
						}
					}
				}
			}
		}
	}

	// 4. Convert to DependencyEdge slice
	edges := make([]models.DependencyEdge, 0, len(edgeCounts))
	for k, w := range edgeCounts {
		edges = append(edges, models.DependencyEdge{
			Source: k.src,
			Target: k.tgt,
			Weight: w,
		})
	}

	// 5. Sort by weight descending and cap for large repos
	sort.Slice(edges, func(i, j int) bool {
		return edges[i].Weight > edges[j].Weight
	})

	const maxEdges = 500
	if len(edges) > maxEdges {
		fmt.Printf("[LOG]   Capping dependency edges from %d to %d (keeping heaviest)\n", len(edges), maxEdges)
		edges = edges[:maxEdges]
	}

	return edges
}

// extractIncludePath pulls the file path from an #include directive string.
// Handles both #include "file.h" and #include <file.h> forms.
func extractIncludePath(include string) string {
	// Try quoted include first: #include "path/to/file.h"
	if idx := strings.Index(include, `"`); idx != -1 {
		end := strings.Index(include[idx+1:], `"`)
		if end != -1 {
			return include[idx+1 : idx+1+end]
		}
	}
	// Try angle-bracket include: #include <path/to/file.h>
	if idx := strings.Index(include, "<"); idx != -1 {
		end := strings.Index(include[idx+1:], ">")
		if end != -1 {
			return include[idx+1 : idx+1+end]
		}
	}
	return ""
}

// resolveInclude tries to match an include path to known files in the repo.
func resolveInclude(incPath, sourceFile string, basenameToFiles map[string][]string) []string {
	// 1. Try relative resolution from the source file's directory
	srcDir := filepath.Dir(sourceFile)
	relResolved := filepath.Join(srcDir, incPath)
	relResolved = filepath.Clean(relResolved)
	// Normalize to forward slashes for consistency
	relResolved = filepath.ToSlash(relResolved)

	// Check if this resolved path is a known file
	base := filepath.Base(incPath)
	candidates := basenameToFiles[base]

	for _, c := range candidates {
		normC := filepath.ToSlash(c)
		if normC == relResolved {
			return []string{c}
		}
	}

	// 2. Try matching by path suffix (handles includes like "raylib/raylib.h")
	normInc := filepath.ToSlash(incPath)
	var matches []string
	for _, c := range candidates {
		normC := filepath.ToSlash(c)
		if strings.HasSuffix(normC, normInc) {
			matches = append(matches, c)
		}
	}
	if len(matches) > 0 {
		return matches
	}

	// 3. Fallback: basename match (for system-like includes within the project)
	if len(candidates) == 1 {
		return candidates
	}

	return nil
}

// Helpers

func cleanJSON(s string) string {
	s = strings.TrimSpace(s)
	if idx := strings.Index(s, "```"); idx != -1 {
		after := s[idx+3:]
		if strings.HasPrefix(after, "json") {
			after = after[4:]
		} else if strings.HasPrefix(after, "JSON") {
			after = after[4:]
		}
		if endIdx := strings.LastIndex(after, "```"); endIdx != -1 {
			s = after[:endIdx]
		} else {
			s = after
		}
		s = strings.TrimSpace(s)
	}
	if idx := findJSONStart(s, '['); idx != -1 {
		if endIdx := strings.LastIndex(s, "]"); endIdx != -1 && endIdx > idx {
			s = s[idx : endIdx+1]
		}
	} else if idx := findJSONStart(s, '{'); idx != -1 {
		if endIdx := strings.LastIndex(s, "}"); endIdx != -1 && endIdx > idx {
			s = s[idx : endIdx+1]
		}
	}
	// Remove line comments and inline comments (e.g., // comment)
	reLineComm := regexp.MustCompile(`(?m)//.*$`)
	s = reLineComm.ReplaceAllString(s, "")
	// Remove block comments (/* ... */)
	reBlockComm := regexp.MustCompile(`(?s)/\*.*?\*/`)
	s = reBlockComm.ReplaceAllString(s, "")
	// Remove illegal trailing commas before closing brackets or braces
	reTrailingComma := regexp.MustCompile(`,\s*([\]}])`)
	s = reTrailingComma.ReplaceAllString(s, "$1")
	return strings.TrimSpace(s)
}

func findJSONStart(s string, char byte) int {
	for i := 0; i < len(s); i++ {
		if s[i] == char {
			for j := i + 1; j < len(s); j++ {
				if s[j] == ' ' || s[j] == '\t' || s[j] == '\r' || s[j] == '\n' {
					continue
				}
				if char == '[' {
					c := s[j]
					if c == '{' || c == '"' || c == '[' || (c >= '0' && c <= '9') || c == '-' || c == 't' || c == 'f' || c == 'n' || c == ']' {
						return i
					}
				} else if char == '{' {
					if s[j] == '"' || s[j] == '}' {
						return i
					}
				}
				break
			}
		}
	}
	return -1
}

func validateTypology(t string) string {
	valid := map[string]bool{
		"core": true, "data": true, "network": true, "security": true,
		"interface": true, "utility": true, "config": true, "test": true,
		"example": true, "unknown": true,
	}
	t = strings.ToLower(strings.TrimSpace(t))
	if valid[t] {
		return t
	}
	return "unknown"
}
