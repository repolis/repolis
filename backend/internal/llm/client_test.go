package llm

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"
)

// Records each request's reasoning_effort, "<absent>" when omitted.
func fakeServer(t *testing.T, content string, efforts *[]string) *httptest.Server {
	t.Helper()
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		var req map[string]any
		_ = json.Unmarshal(body, &req)
		effort, ok := req["reasoning_effort"].(string)
		if !ok {
			effort = "<absent>"
		}
		*efforts = append(*efforts, effort)
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(map[string]any{
			"choices": []map[string]any{{
				"message":       map[string]any{"role": "assistant", "content": content},
				"finish_reason": "stop",
			}},
		})
	}))
	t.Cleanup(srv.Close)
	return srv
}

func newTestClient(t *testing.T, baseURL string, cache *Cache) *Client {
	t.Helper()
	t.Setenv("LLM_BASE_URL", baseURL)
	t.Setenv("LLM_MODEL", "test-model")
	c, err := NewClient(cache)
	if err != nil {
		t.Fatal(err)
	}
	return c
}

// Every task caps output at 8-220 tokens, so a model left to think returns
// nothing at all. Thinking must be off unless explicitly configured.
func TestReasoningEffortDefaultsToNone(t *testing.T) {
	var efforts []string
	srv := fakeServer(t, "Vdbe", &efforts)

	c := newTestClient(t, srv.URL, nil)
	if _, err := c.complete(context.Background(), c.adjudicateRequest("c", "f", "()", []string{"A", "B"})); err != nil {
		t.Fatal(err)
	}

	t.Setenv("LLM_REASONING_EFFORT", "minimal")
	c = newTestClient(t, srv.URL, nil)
	if _, err := c.complete(context.Background(), c.adjudicateRequest("c", "f", "()", []string{"A", "B"})); err != nil {
		t.Fatal(err)
	}

	t.Setenv("LLM_REASONING_EFFORT", "default")
	c = newTestClient(t, srv.URL, nil)
	if _, err := c.complete(context.Background(), c.adjudicateRequest("c", "f", "()", []string{"A", "B"})); err != nil {
		t.Fatal(err)
	}

	want := []string{"none", "minimal", "<absent>"}
	if len(efforts) != len(want) {
		t.Fatalf("got %d requests, want %d", len(efforts), len(want))
	}
	for i := range want {
		if efforts[i] != want[i] {
			t.Errorf("request %d sent reasoning_effort %q, want %q", i, efforts[i], want[i])
		}
	}
}

// A thinking model's empty reply must surface as an error and never be
// cached, or the fallback it causes would outlive the fix.
func TestEmptyContentIsErrorAndNotCached(t *testing.T) {
	var efforts []string
	srv := fakeServer(t, "", &efforts)
	cache := NewCache(nil)
	c := newTestClient(t, srv.URL, cache)

	req := c.adjudicateRequest("c", "f", "()", []string{"A", "B"})
	if _, err := c.complete(context.Background(), req); err == nil {
		t.Fatal("empty content returned no error")
	}
	req.effort = c.reasoningEffort
	if _, ok := cache.Get(req.hash()); ok {
		t.Error("empty content was cached")
	}
}

func TestCacheKeyIncludesReasoningEffort(t *testing.T) {
	r := request{kind: "k", model: "m", user: "u", maxTokens: 8}
	off := r
	off.effort = "none"
	if r.hash() == off.hash() {
		t.Error("reasoning effort does not change the cache key")
	}
}

func TestAdjudicationCanBeDisabled(t *testing.T) {
	var efforts []string
	srv := fakeServer(t, "Vdbe", &efforts)
	t.Setenv("LLM_ADJUDICATE", "false")
	c := newTestClient(t, srv.URL, nil)

	if n := c.AdjudicateAssociations(context.Background(), nil, nil); n != 0 {
		t.Errorf("disabled adjudication changed %d associations", n)
	}
	if len(efforts) != 0 {
		t.Errorf("disabled adjudication made %d requests", len(efforts))
	}
}

// Google's free tier answers over-quota with a 429 whose body is a JSON array
// that go-openai cannot decode; the call must still be retried, not dropped.
func TestRateLimitedCallIsRetried(t *testing.T) {
	calls := 0
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls++
		w.Header().Set("Content-Type", "application/json")
		if calls == 1 {
			w.WriteHeader(http.StatusTooManyRequests)
			_, _ = io.WriteString(w, `[{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"0s"}]}}]`)
			return
		}
		_ = json.NewEncoder(w).Encode(map[string]any{
			"choices": []map[string]any{{"message": map[string]any{"role": "assistant", "content": "Vdbe"}, "finish_reason": "stop"}},
		})
	}))
	t.Cleanup(srv.Close)

	c := newTestClient(t, srv.URL, nil)
	out, err := c.complete(context.Background(), c.adjudicateRequest("c", "f", "()", []string{"A", "B"}))
	if err != nil || out != "Vdbe" {
		t.Fatalf("got %q, %v after a 429; want the retried answer", out, err)
	}
	if calls != 2 || c.Retries() != 1 {
		t.Errorf("calls=%d retries=%d, want 2 and 1", calls, c.Retries())
	}
}
