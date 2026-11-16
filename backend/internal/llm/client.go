package llm

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"net/http"
	"os"
	"regexp"
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
	richModel string

	// Sent as reasoning_effort on every call. Every task here caps output at
	// 8-220 tokens, so a model left to think spends the whole budget inside
	// its reasoning and returns empty content.
	reasoningEffort string
	warnedEmpty     atomic.Bool

	// When false the association rules decide alone. On the hand-labelled
	// residual set qwen3.5-2b and gemma4-12b scored below the rules (43-51%
	// against 77%), so a weak model is worse than no model here.
	adjudicate bool

	concurrency int
	timeout     time.Duration
	cache       *Cache

	calls     atomic.Int64
	retries   atomic.Int64
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
		apiKey = "llama.cpp"
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

	// Providers disagree on the value that disables thinking: llama-server needs
	// "none" (it still thinks at "minimal"), while gemini-3.5-flash-lite
	// rejects "none" and only accepts "minimal". "default" omits the field.
	effort := "none"
	if v, ok := os.LookupEnv("LLM_REASONING_EFFORT"); ok {
		effort = strings.TrimSpace(v)
	}
	if effort == "default" {
		effort = ""
	}

	adjudicate := true
	if v := os.Getenv("LLM_ADJUDICATE"); v != "" {
		if b, err := strconv.ParseBool(v); err == nil {
			adjudicate = b
		}
	}

	logger.Log(logger.InfoLevel, "LLM ready: fast=%s rich=%s concurrency=%d reasoning=%q adjudicate=%v",
		fast, rich, concurrency, effort, adjudicate)
	return &Client{
		api:             openai.NewClientWithConfig(cfg),
		fastModel:       fast,
		richModel:       rich,
		reasoningEffort: effort,
		adjudicate:      adjudicate,
		concurrency:     concurrency,
		timeout:         timeout,
		cache:           cache,
	}, nil
}

func (c *Client) Calls() int64     { return c.calls.Load() }
func (c *Client) CacheHits() int64 { return c.cacheHits.Load() }

func (c *Client) Retries() int64 { return c.retries.Load() }

type request struct {
	model     string
	effort    string
	system    string
	user      string
	maxTokens int
	stop      []string
	kind      string
}

func (r request) hash() string {
	h := sha256.New()
	h.Write([]byte(r.kind + "\x00" + r.model + "\x00" + r.system + "\x00" + r.user))
	if r.effort != "" {
		fmt.Fprintf(h, "\x00effort=%s", r.effort)
	}
	fmt.Fprintf(h, "\x00%d", r.maxTokens)
	return hex.EncodeToString(h.Sum(nil))
}

func (c *Client) complete(ctx context.Context, r request) (string, error) {
	r.effort = c.reasoningEffort
	key := r.hash()
	if c.cache != nil && !c.bypassCache.Load() {
		if v, ok := c.cache.Get(key); ok {
			c.cacheHits.Add(1)
			return v, nil
		}
	}

	msgs := make([]openai.ChatCompletionMessage, 0, 2)
	if r.system != "" {
		msgs = append(msgs, openai.ChatCompletionMessage{Role: openai.ChatMessageRoleSystem, Content: r.system})
	}
	msgs = append(msgs, openai.ChatCompletionMessage{Role: openai.ChatMessageRoleUser, Content: r.user})

	creq := openai.ChatCompletionRequest{
		Model:    r.model,
		Messages: msgs,
		// Bounded: unbounded, a small model restates its reasoning for
		// hundreds of tokens on every call.
		Temperature:     0.0,
		TopP:            1.0,
		MaxTokens:       r.maxTokens,
		Stop:            r.stop,
		ReasoningEffort: r.effort,
	}

	var resp openai.ChatCompletionResponse
	var err error
	for attempt := 1; ; attempt++ {
		c.calls.Add(1)
		callCtx, cancel := context.WithTimeout(ctx, c.timeout)
		resp, err = c.api.CreateChatCompletion(callCtx, creq)
		cancel()
		wait, retry := retryAfter(err, attempt)
		if !retry {
			break
		}
		c.retries.Add(1)
		logger.Log(logger.DebugLevel, "%s rate limited, retrying in %s (attempt %d)", r.model, wait, attempt)
		select {
		case <-time.After(wait):
		case <-ctx.Done():
			return "", ctx.Err()
		}
	}
	if err != nil {
		return "", err
	}
	if len(resp.Choices) == 0 {
		return "", fmt.Errorf("empty completion")
	}

	out := strings.TrimSpace(resp.Choices[0].Message.Content)
	if out == "" {
		// Every caller treats "" as "no answer" and falls back silently, so
		// without this a thinking model disables the LLM with no visible sign.
		// The finish reason does not identify it: a "\n" stop sequence ends
		// the reasoning early and reports "stop", not "length".
		if c.warnedEmpty.CompareAndSwap(false, true) {
			logger.Log(logger.WarnLevel,
				"%s returned no content (finish_reason=%s); it is probably thinking. Check LLM_REASONING_EFFORT (current %q)",
				r.model, resp.Choices[0].FinishReason, r.effort)
		}
		return "", fmt.Errorf("empty content (finish_reason=%s)", resp.Choices[0].FinishReason)
	}
	if c.cache != nil {
		c.cache.Put(key, out)
	}
	return out, nil
}

const maxAttempts = 6

var retryDelayRe = regexp.MustCompile(`"retryDelay":\s*"(\d+(?:\.\d+)?)s"`)

// Hosted APIs answer over-quota with 429 (Google's free tier allows 15 requests
// a minute per model); without a retry each one silently becomes a rule or
// directory fallback. Google states the wait in the body.
func retryAfter(err error, attempt int) (time.Duration, bool) {
	if err == nil || attempt >= maxAttempts {
		return 0, false
	}
	status := 0
	var body []byte
	var apiErr *openai.APIError
	var reqErr *openai.RequestError
	switch {
	case errors.As(err, &apiErr):
		status = apiErr.HTTPStatusCode
	case errors.As(err, &reqErr):
		status, body = reqErr.HTTPStatusCode, reqErr.Body
	}
	if status != http.StatusTooManyRequests && status != http.StatusServiceUnavailable && status != 529 {
		return 0, false
	}
	if m := retryDelayRe.FindSubmatch(body); m != nil {
		if secs, perr := strconv.ParseFloat(string(m[1]), 64); perr == nil {
			return time.Duration(secs*float64(time.Second)) + 500*time.Millisecond, true
		}
	}
	return time.Duration(1<<attempt) * time.Second, true
}

// Results keep input order; progress is reported as jobs finish.
func runBatch[T any](ctx context.Context, concurrency, n int, progress func(done, total int), fn func(i int) T) []T {
	results := make([]T, n)
	sem := make(chan struct{}, max(concurrency, 1))
	var wg sync.WaitGroup
	var done atomic.Int64
	for i := range n {
		wg.Go(func() {
			select {
			case sem <- struct{}{}:
			case <-ctx.Done():
				return
			}
			defer func() { <-sem }()
			results[i] = fn(i)
			if progress != nil {
				progress(int(done.Add(1)), n)
			}
		})
	}
	wg.Wait()
	return results
}

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
