// Package llm is the thin, bounded LLM layer.
//
// Design rules, enforced throughout:
//   - The model is never asked to partition, count or enumerate. It picks one
//     item from a closed list that is already in its prompt.
//   - Every prompt is small and self-contained. No source snippets, no
//     repo-wide symbol dumps.
//   - Every response is bounded by MaxTokens and validated against the closed
//     set; anything unrecognised is discarded, never guessed at.
//   - Every result is cached by content hash, so re-analysis costs nothing.
package llm

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/repolis/repolis/backend/internal/logger"
	openai "github.com/sashabaranov/go-openai"
)

type Client struct {
	api *openai.Client

	// fastModel handles closed-set selection (association, district naming).
	// A 1.5B model is sufficient once the candidate list is short and
	// AST-verified, and it is several times faster than a 12B one.
	fastModel string
	// richModel is used only for on-demand building explanations, where the
	// prompt can afford real source context and the user is waiting for one
	// answer rather than thousands.
	richModel string

	concurrency int
	timeout     time.Duration
	cache       *Cache

	calls     atomic.Int64
	cacheHits atomic.Int64
	// bypassCache makes this run ignore stored answers. Writes still happen,
	// so a forced regeneration refreshes the cache rather than disabling it.
	bypassCache atomic.Bool
}

// SetCacheBypass forces every question to go to the model, ignoring any
// answer already stored for the same prompt.
func (c *Client) SetCacheBypass(v bool) { c.bypassCache.Store(v) }

func NewClient(cache *Cache) (*Client, error) {
	baseURL := os.Getenv("LLM_BASE_URL")
	if baseURL == "" {
		return nil, fmt.Errorf("LLM_BASE_URL is not set")
	}

	rich := os.Getenv("LLM_MODEL")
	if rich == "" {
		return nil, fmt.Errorf("LLM_MODEL is not set")
	}
	fast := os.Getenv("LLM_MODEL_FAST")
	if fast == "" {
		fast = rich
	}

	apiKey := os.Getenv("LLM_API_KEY")
	if apiKey == "" {
		apiKey = "ollama"
	}

	cfg := openai.DefaultConfig(apiKey)
	cfg.BaseURL = baseURL

	concurrency := 2
	if v := os.Getenv("LLM_CONCURRENCY"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n > 0 && n <= 16 {
			concurrency = n
		}
	}

	timeout := 90 * time.Second
	if v := os.Getenv("LLM_TIMEOUT_SECONDS"); v != "" {
		if n, err := strconv.Atoi(v); err == nil && n > 0 {
			timeout = time.Duration(n) * time.Second
		}
	}

	logger.Log(logger.InfoLevel, "LLM ready: fast=%s rich=%s concurrency=%d", fast, rich, concurrency)
	return &Client{
		api:         openai.NewClientWithConfig(cfg),
		fastModel:   fast,
		richModel:   rich,
		concurrency: concurrency,
		timeout:     timeout,
		cache:       cache,
	}, nil
}

func (c *Client) Calls() int64     { return c.calls.Load() }
func (c *Client) CacheHits() int64 { return c.cacheHits.Load() }

type request struct {
	model     string
	system    string
	user      string
	maxTokens int
	stop      []string
	kind      string
}

func (r request) hash() string {
	h := sha256.New()
	h.Write([]byte(r.kind + "\x00" + r.model + "\x00" + r.system + "\x00" + r.user))
	fmt.Fprintf(h, "\x00%d", r.maxTokens)
	return hex.EncodeToString(h.Sum(nil))
}

// complete runs one bounded completion, returning cached output when available.
func (c *Client) complete(ctx context.Context, r request) (string, error) {
	key := r.hash()
	if c.cache != nil && !c.bypassCache.Load() {
		if v, ok := c.cache.Get(key); ok {
			c.cacheHits.Add(1)
			return v, nil
		}
	}

	callCtx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()

	msgs := make([]openai.ChatCompletionMessage, 0, 2)
	if r.system != "" {
		msgs = append(msgs, openai.ChatCompletionMessage{Role: openai.ChatMessageRoleSystem, Content: r.system})
	}
	msgs = append(msgs, openai.ChatCompletionMessage{Role: openai.ChatMessageRoleUser, Content: r.user})

	c.calls.Add(1)
	resp, err := c.api.CreateChatCompletion(callCtx, openai.ChatCompletionRequest{
		Model:    r.model,
		Messages: msgs,
		// Deterministic, and bounded. The previous code set no MaxTokens at
		// all, so a small model could emit hundreds of tokens of restated
		// reasoning on every one of ~1000 per-file calls.
		Temperature: 0.0,
		TopP:        1.0,
		MaxTokens:   r.maxTokens,
		Stop:        r.stop,
	})
	if err != nil {
		return "", err
	}
	if len(resp.Choices) == 0 {
		return "", fmt.Errorf("empty completion")
	}

	out := strings.TrimSpace(resp.Choices[0].Message.Content)
	if c.cache != nil {
		c.cache.Put(key, out)
	}
	return out, nil
}

// runBatch executes jobs with bounded concurrency, preserving input order.
func runBatch[T any](ctx context.Context, concurrency int, n int, fn func(i int) T) []T {
	results := make([]T, n)
	if n == 0 {
		return results
	}
	if concurrency < 1 {
		concurrency = 1
	}

	sem := make(chan struct{}, concurrency)
	var wg sync.WaitGroup
	for i := 0; i < n; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			select {
			case sem <- struct{}{}:
			case <-ctx.Done():
				return
			}
			defer func() { <-sem }()
			results[i] = fn(i)
		}(i)
	}
	wg.Wait()
	return results
}

// stripThinking removes <think>...</think> blocks that reasoning-tuned local
// models emit before their answer.
func stripThinking(s string) string {
	for {
		start := strings.Index(s, "<think>")
		if start == -1 {
			break
		}
		end := strings.Index(s, "</think>")
		if end == -1 || end < start {
			s = s[:start]
			break
		}
		s = s[:start] + s[end+len("</think>"):]
	}
	return strings.TrimSpace(s)
}
