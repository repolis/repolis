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

// adjudicateSystem frames one decision at a time.
//
// Batching was tried first and abandoned. Measured against qwen2.5:1.5b on the
// residual set of tsoding/nothing, multi-item prompts silently dropped the
// required `N.` numbering and collapsed the whole batch to a single bare word,
// losing every answer; the failure was sensitive to unrelated details of the
// prompt, so it could not be tuned away. One item per call is trivially
// formatted, scored 11/11 valid on the same set, and costs 68 ms each because
// the output is three tokens. A batch of six saves prompt overhead that the
// decode time dwarfs anyway.
const adjudicateSystemFmt = `You are given a %s function and a list of type names.
Reply with exactly one name copied from the list, or NONE.
Pick the type the function reads or modifies most: usually the one it is named after, or the type of its first pointer parameter.
Reply with the name only. No punctuation, no explanation.`

// AdjudicateAssociations resolves the associations the deterministic rules
// could not settle.
//
// Only genuinely ambiguous functions reach this point - typically where the
// naming convention and the first-parameter convention disagree, such as
// sqlite's `static int vdbeCommit(sqlite3 *db, Vdbe *p)`. On tsoding/nothing
// the rules settle 485 of 496 functions for free, leaving 11 here.
//
// Every answer is checked twice: it must be a member of the candidate list
// shown in the prompt, and it must survive the AST verifier. An unverifiable
// answer becomes an orphan rather than a wrong building height.
func (c *Client) AdjudicateAssociations(ctx context.Context, st *analyzer.SymbolTable, progress func(done, total int)) int {
	residual := st.Residual()
	if len(residual) == 0 {
		return 0
	}

	// Budget. On libgit2 the rules leave ~2500 ambiguous functions; at roughly
	// ten decisions a second that is four minutes of a user watching a city
	// they can already use. Residual() orders by how much a wrong answer
	// distorts the picture, and every entry already carries a tentative
	// rule-based answer, so truncating costs accuracy on the least
	// consequential cases rather than losing them.
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

	var done int
	answers := runBatch(ctx, c.concurrency, len(questions), func(i int) string {
		q := questions[i]
		user := fmt.Sprintf("Function: %s%s\nChoices: %s, NONE\nAnswer:",
			q.assoc.Fn.Name, truncate(q.assoc.Fn.Signature, 100),
			strings.Join(q.options, ", "))

		out, err := c.complete(ctx, request{
			kind:      "assoc.v3",
			model:     c.fastModel,
			system:    fmt.Sprintf(adjudicateSystemFmt, st.Language()),
			user:      user,
			maxTokens: 8,
			stop:      []string{"\n"},
		})
		if progress != nil {
			done++
			progress(done, len(questions))
		}
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
			st.ClearTentative(questions[i].assoc)
			detached++
		default:
			id, known := questions[i].nameToID[ans]
			if !known {
				rejected++
				continue
			}
			if st.ApplyLLM(questions[i].assoc, id) {
				changed++
			} else {
				confirmed++
			}
		}
	}

	logger.Log(logger.InfoLevel,
		"Adjudication: %d reassigned, %d confirmed, %d detached, %d rejected by verifier",
		changed, confirmed, detached, rejected)
	return changed
}

func cleanAnswer(s string) string {
	s = stripThinking(s)
	if i := strings.IndexAny(s, "\n\r"); i != -1 {
		s = s[:i]
	}
	return strings.Trim(strings.TrimSpace(s), " .,;:`\"'*")
}

func truncate(s string, n int) string {
	s = strings.TrimSpace(s)
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
