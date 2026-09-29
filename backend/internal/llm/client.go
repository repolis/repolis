// Package llm is the thin, bounded LLM layer. The model only ever picks one
// item from a closed list already in its prompt; prompts carry no source or
// repo-wide dumps, replies are capped by MaxTokens and validated against that
// list, and every result is cached by content hash.
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

	// Closed-set selection: a 1.5B model suffices once candidates are short
	// and AST-verified, and beats a 12B one several times over on speed.
	fastModel string
	// On-demand building explanations only, where the prompt can afford real
	// source context and one user waits for one answer.
	richModel string

	concurrency int
	timeout     time.Duration
	cache       *Cache

	calls     atomic.Int64
	cacheHits atomic.Int64
	// Ignore stored answers for this run. Writes still happen, so a forced
	// regeneration refreshes the cache rather than disabling it.
	bypassCache atomic.Bool
}

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
		// Bounded: unbounded, a small model restates its reasoning for
		// hundreds of tokens on every call.
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

// stripThinking removes the <think> blocks reasoning-tuned models emit.
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
