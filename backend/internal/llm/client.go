package llm

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

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

func (c *Client) CategorizeCity(ctx context.Context, clonePath string, city *models.CityMap) {
	fmt.Println("[LOG] Starting 2-Pass LLM Typology & Summary Analysis...")
	for i := range city.Files {
		err := c.categorizeFile(ctx, clonePath, &city.Files[i])
		if err != nil {
			fmt.Printf("[WARNING] LLM failed to categorize %s: %v\n", city.Files[i].Path, err)
			city.Files[i].Typology = "unknown"
			city.Files[i].Summary = "Analysis failed"
			city.Files[i].Tags = []string{}
		}
	}
	fmt.Println("[LOG] LLM Analysis Complete.")
}

func (c *Client) categorizeFile(ctx context.Context, clonePath string, file *models.FileMetrics) error {
	var crucialSymbols []string
	if len(file.FunctionNames) > 0 || len(file.StructNames) > 0 {
		prompt1 := fmt.Sprintf(
			"Identify the 1 to 3 most important functions OR structs that define the core logic of this file.\n"+
				"Output ONLY a comma-separated list of names. No conversational text, no markdown. Example output: main,UserConfig\n\n"+
				"File: %s\nFunctions: %v\nStructs: %v",
			file.Path, file.FunctionNames, file.StructNames,
		)

		resp1, err := c.api.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
			Model: c.model,
			Messages: []openai.ChatCompletionMessage{
				{Role: openai.ChatMessageRoleUser, Content: prompt1},
			},
			MaxTokens:   30,
			Temperature: 0.1,
		})

		if err == nil && len(resp1.Choices) > 0 {
			for name := range strings.SplitSeq(resp1.Choices[0].Message.Content, ",") {
				name = strings.TrimSpace(name)
				if name != "" {
					crucialSymbols = append(crucialSymbols, name)
				}
			}
		}
	}

	var symbolBodies string
	if len(crucialSymbols) > 0 {
		fullPath := filepath.Join(clonePath, file.Path)
		symbolBodies = analyzer.ExtractSymbols(fullPath, crucialSymbols)
	}

	systemPrompt := `You are an AI code analyzer evaluating files from an unknown software repository.
You must output a strictly valid JSON object matching this schema:
{
  "typology": "...",
  "summary": "5 to 10 word plain-English explanation of what this specific file does.",
  "tags": ["tag1", "tag2", "tag3"]
}

The "typology" MUST be EXACTLY ONE of these strings: core, data, network, security, interface, utility, config, test, example, unknown.

RULES:
1. "summary" must describe the actual code (e.g. "Mathematical expression parser and evaluator"). Do NOT mention that you are an AI or an analyzer.
2. "tags" should be 3 short technical keywords (e.g. ["math", "parser", "ast"]).
3. If the file path contains "test", "bench", "smoke", "spec", or "mock", the typology MUST be "test".
4. If the file path contains "example" or "demo", the typology MUST be "example".
5. Output ONLY raw valid JSON. No markdown formatting, no backticks, no explanations.`

	prompt2 := fmt.Sprintf(
		"File Path: %s\nFunctions: %v\nStructs: %v\nIncludes: %v\nStrings: %v\n\nCrucial Code Context (minified):\n%s\n\nBased on the above, provide the JSON analysis.",
		file.Path, file.FunctionNames, file.StructNames, file.Includes, file.StringLiterals, symbolBodies,
	)

	resp2, err := c.api.CreateChatCompletion(ctx, openai.ChatCompletionRequest{
		Model: c.model,
		Messages: []openai.ChatCompletionMessage{
			{Role: openai.ChatMessageRoleSystem, Content: systemPrompt},
			{Role: openai.ChatMessageRoleUser, Content: prompt2},
		},
		ResponseFormat: &openai.ChatCompletionResponseFormat{
			Type: openai.ChatCompletionResponseFormatTypeJSONObject,
		},
		Temperature: 0.0,
	})

	if err != nil {
		return err
	}

	if len(resp2.Choices) > 0 {
		content := strings.TrimSpace(resp2.Choices[0].Message.Content)

		if content, ok := strings.CutPrefix(content, "```json"); ok {
			content = strings.TrimSuffix(content, "```")
			content = strings.TrimSpace(content)
		} else if content, ok := strings.CutPrefix(content, "```"); ok {
			content = strings.TrimSuffix(content, "```")
			content = strings.TrimSpace(content)
		}

		var result struct {
			Typology string   `json:"typology"`
			Summary  string   `json:"summary"`
			Tags     []string `json:"tags"`
		}

		if jsonErr := json.Unmarshal([]byte(content), &result); jsonErr == nil {
			validEnums := map[string]bool{
				"core": true, "data": true, "network": true, "security": true,
				"interface": true, "utility": true, "config": true, "test": true,
				"example": true, "unknown": true,
			}

			if validEnums[result.Typology] {
				file.Typology = result.Typology
			} else {
				file.Typology = "unknown"
			}
			file.Summary = result.Summary
			file.Tags = result.Tags
		} else {
			file.Typology = "unknown"
			file.Summary = "Failed to parse JSON"
			file.Tags = []string{}
		}
	}
	return nil
}
