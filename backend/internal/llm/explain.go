package llm

import (
	"context"
	"fmt"
	"strings"

	"github.com/repolis/repolis/backend/internal/models"
)

const explainSystemFmt = `You explain %s code entities to a developer exploring an unfamiliar codebase.
Write 2-3 short sentences: what it represents, what it is used for, and anything notable.
Plain prose. No markdown, no bullet points, no preamble.`

// ExplainBuilding produces an on-demand description of a single building.
//
// This is the only place the larger model is used, and it is never on the
// critical path: the city renders fully without it. Generating five-word
// summaries for every building up front cost ~54 calls per repo to produce
// text nobody read; one good explanation when the user actually clicks
// something is worth far more.
func (c *Client) ExplainBuilding(ctx context.Context, b models.Building, source string, callers, callees []string) (string, error) {
	language := b.Language
	if language == "" {
		language = "source"
	}
	var p strings.Builder
	fmt.Fprintf(&p, "Entity: %s (%s)\nFile: %s\n", b.Name, b.Kind, b.SourceFile)
	if len(b.Fields) > 0 {
		fmt.Fprintf(&p, "Fields (%d): %s\n", b.NumFields, truncate(strings.Join(b.Fields, ", "), 400))
	}
	if len(b.Methods) > 0 {
		fmt.Fprintf(&p, "Functions (%d): %s\n", b.NumMethods, truncate(strings.Join(b.Methods, ", "), 400))
	}
	if len(callees) > 0 {
		fmt.Fprintf(&p, "Depends on: %s\n", truncate(strings.Join(callees, ", "), 200))
	}
	if len(callers) > 0 {
		fmt.Fprintf(&p, "Used by: %s\n", truncate(strings.Join(callers, ", "), 200))
	}
	if b.CommitChurn > 0 {
		fmt.Fprintf(&p, "History: %d commits, last touched %d days ago\n", b.CommitChurn, b.AgeDays)
	}
	if source != "" {
		fmt.Fprintf(&p, "\nSource:\n%s\n", truncate(source, 2500))
	}

	out, err := c.complete(ctx, request{
		kind:      "explain.v1",
		model:     c.richModel,
		system:    fmt.Sprintf(explainSystemFmt, language),
		user:      p.String(),
		maxTokens: 220,
	})
	if err != nil {
		return "", err
	}
	return stripThinking(out), nil
}
