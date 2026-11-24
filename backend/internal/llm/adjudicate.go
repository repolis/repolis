package llm

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/repolis/repolis/backend/internal/analyzer"
	"github.com/repolis/repolis/backend/internal/logger"
)

// One decision per call. Batching was measured and abandoned: qwen2.5:1.5b
// dropped the required numbering and collapsed a whole batch to one bare word,
// unpredictably. One item scored 11/11 valid at 68 ms each, since the output
// is three tokens and prompt overhead is dwarfed by decode time.
const adjudicateSystemFmt = `You are given a %s function and a list of type names.
Reply with exactly one name copied from the list, or NONE.
Pick the type the function reads or modifies most: usually the one it is named after, or the type of its first pointer parameter.
Reply with the name only. No punctuation, no explanation.`

// Every answer must both appear in the candidate list and survive the AST
// verifier; an unverifiable one becomes an orphan, not a wrong height.
func (c *Client) AdjudicateAssociations(ctx context.Context, st *analyzer.SymbolTable, progress func(done, total int)) int {
	if !c.adjudicate {
		logger.Log(logger.InfoLevel, "LLM adjudication disabled (LLM_ADJUDICATE=false); association rules decide alone")
		return 0
	}
	residual := st.Residual()
	if len(residual) == 0 {
		return 0
	}

	// libgit2 leaves ~2500 ambiguous functions, four minutes at ten a second.
	// Residual() is ordered by impact and every entry already has a tentative
	// answer, so truncating costs accuracy only on the least consequential.
	budget := 600
	if v := os.Getenv("LLM_ASSOC_BUDGET"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n >= 0 {
			budget = n
		}
	}
	skipped := 0
	if budget > 0 && len(residual) > budget {
		skipped = len(residual) - budget
		residual = residual[:budget]
	}

	type question struct {
		assoc    *analyzer.Assoc
		options  []string
		nameToID map[string]string
	}

	questions := make([]question, 0, len(residual))
	for _, a := range residual {
		q := question{assoc: a, nameToID: map[string]string{}}
		for _, cid := range a.Candidates {
			name := st.TypeName(cid)
			if name == "" {
				continue
			}
			if _, clash := q.nameToID[name]; clash {
				continue
			}
			q.nameToID[name] = cid
			q.options = append(q.options, name)
		}
		if len(q.options) >= 2 {
			questions = append(questions, q)
		}
	}
	if len(questions) == 0 {
		return 0
	}

	logger.Log(logger.InfoLevel,
		"Adjudicating %d ambiguous associations (%d settled confidently by rule, %d kept at rule default over budget)",
		len(questions), len(st.Assocs)-len(questions)-skipped, skipped)

	answers := runBatch(ctx, c.concurrency, len(questions), progress, func(i int) string {
		q := questions[i]
		out, err := c.complete(ctx, c.adjudicateRequest(st.Language(), q.assoc.Fn.Name, q.assoc.Fn.Signature, q.options))
		if err != nil {
			logger.Log(logger.DebugLevel, "Adjudication failed for %s: %v", q.assoc.Fn.Name, err)
			return ""
		}
		if os.Getenv("LLM_DEBUG") != "" {
			logger.Log(logger.WarnLevel, "%s [%s] -> %q", q.assoc.Fn.Name, strings.Join(q.options, "|"), out)
		}
		return cleanAnswer(out)
	})

	changed, confirmed, detached, rejected := 0, 0, 0, 0
	for i, ans := range answers {
		switch {
		case ans == "":
			// No answer at all: keep the rule's tentative choice.
		case ans == "NONE":
			questions[i].assoc.Detach()
			detached++
		default:
			// An unknown name is a rejection like any other.
			switch st.ApplyLLM(questions[i].assoc, questions[i].nameToID[ans]) {
			case analyzer.LLMApplied:
				changed++
			case analyzer.LLMUnchanged:
				confirmed++
			case analyzer.LLMRejected:
				rejected++
			}
		}
	}

	logger.Log(logger.InfoLevel,
		"Adjudication: %d reassigned, %d confirmed, %d detached, %d rejected",
		changed, confirmed, detached, rejected)
	return changed
}

func (c *Client) adjudicateRequest(language, fn, signature string, options []string) request {
	return request{
		kind:   "assoc.v3",
		model:  c.fastModel,
		system: fmt.Sprintf(adjudicateSystemFmt, language),
		user: fmt.Sprintf("Function: %s%s\nChoices: %s, NONE\nAnswer:",
			fn, truncate(signature, 100), strings.Join(options, ", ")),
		// The "\n" stop ends a good answer after 2-6 tokens. The headroom is
		// for long identifiers and for providers that bill hidden reasoning
		// against the cap: at 8, gemini-3.5-flash-lite returned "git_diff_".
		maxTokens: 16,
		stop:      []string{"\n"},
	}
}
