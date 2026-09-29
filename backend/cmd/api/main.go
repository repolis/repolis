package main

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/joho/godotenv"

	"github.com/repolis/repolis/backend/internal/analyzer"
	"github.com/repolis/repolis/backend/internal/db"
	"github.com/repolis/repolis/backend/internal/git"
	"github.com/repolis/repolis/backend/internal/jobs"
	"github.com/repolis/repolis/backend/internal/llm"
	"github.com/repolis/repolis/backend/internal/logger"
	"github.com/repolis/repolis/backend/internal/models"
	"github.com/repolis/repolis/backend/internal/pipeline"
)

// cacheSchema is bumped whenever the CityMap shape changes, so a stale cache
// entry can never be served to a renderer that expects different fields.
const cacheSchema = "v2"

const (
	maxBodyBytes  = 64 << 10
	cloneMaxAge   = 24 * time.Hour
	analysisLimit = 20 * time.Minute
)

type server struct {
	jobs  *jobs.Manager
	cache *llm.Cache
}

type contextKey string

const userIDKey contextKey = "userID"

func CookieMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cookie, err := r.Cookie("repolis_user_id")
		var userID string
		if err != nil {
			userID = uuid.New().String()
			http.SetCookie(w, &http.Cookie{
				Name:     "repolis_user_id",
				Value:    userID,
				Path:     "/",
				HttpOnly: true,
				Secure:   os.Getenv("APP_ENV") == "production",
				SameSite: http.SameSiteLaxMode,
				Expires:  time.Now().Add(365 * 24 * time.Hour),
			})
		} else {
			userID = cookie.Value
		}
		next.ServeHTTP(w, r.WithContext(context.WithValue(r.Context(), userIDKey, userID)))
	})
}

// CORSMiddleware allows the frontend to run on a different origin than the API.
// In development Vite proxies /api so this is a no-op; without it any deployment
// that does not proxy is broken.
func CORSMiddleware(next http.Handler) http.Handler {
	allowed := os.Getenv("CORS_ORIGIN")
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if allowed != "" {
			origin := r.Header.Get("Origin")
			if origin == allowed || allowed == "*" {
				w.Header().Set("Access-Control-Allow-Origin", origin)
				w.Header().Set("Access-Control-Allow-Credentials", "true")
				w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
				w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
			}
		}
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func main() {
	_ = godotenv.Load()

	if err := db.InitDB(); err != nil {
		logger.Log(logger.FatalLevel, "Failed to initialize database: %v", err)
	}
	if err := os.MkdirAll(filepath.Join("data", "cities"), 0o755); err != nil {
		logger.Log(logger.FatalLevel, "Failed to create cache directory: %v", err)
	}

	s := &server{jobs: jobs.NewManager(), cache: llm.NewCache(db.DB)}
	go reapStaleClones()

	mux := http.NewServeMux()
	mux.HandleFunc("POST /api/analyze", s.handleAnalyze)
	mux.HandleFunc("GET /api/jobs/{id}/events", s.handleJobEvents)
	mux.HandleFunc("POST /api/explain", s.handleExplain)
	mux.HandleFunc("GET /api/health", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
	})

	srv := &http.Server{
		Addr:              ":8080",
		Handler:           CORSMiddleware(CookieMiddleware(mux)),
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       30 * time.Second,
		// No WriteTimeout: /api/jobs/{id}/events is a long-lived SSE stream.
		IdleTimeout:    120 * time.Second,
		MaxHeaderBytes: 1 << 16,
	}

	logger.Log(logger.InfoLevel, "Backend server is running on http://localhost:8080")
	if err := srv.ListenAndServe(); err != nil {
		logger.Log(logger.FatalLevel, "Server crashed: %v", err)
	}
}

func reapStaleClones() {
	clean := func() {
		for id, path := range db.StaleClones(cloneMaxAge) {
			if path != "" && strings.Contains(path, "repolis-session-") {
				_ = os.RemoveAll(path)
			}
			db.DeleteSession(id)
		}
	}
	clean()
	for range time.Tick(time.Hour) {
		clean()
	}
}

func cacheKey(repoURL, commit string) string {
	return fmt.Sprintf("%x", sha256.Sum256([]byte(cacheSchema+"|"+repoURL+"@"+commit)))
}

func cachePath(key string) string { return filepath.Join("data", "cities", key+".json") }

func readCachedCity(key string) *models.CityMap {
	b, err := os.ReadFile(cachePath(key))
	if err != nil {
		return nil
	}
	var city models.CityMap
	if json.Unmarshal(b, &city) != nil {
		return nil
	}
	return &city
}

// writeCachedCity writes atomically; the previous direct WriteFile could be
// read half-written by a concurrent request.
func writeCachedCity(key string, city *models.CityMap) {
	b, err := json.Marshal(city)
	if err != nil {
		return
	}
	tmp := cachePath(key) + ".tmp"
	if os.WriteFile(tmp, b, 0o644) != nil {
		return
	}
	if err := os.Rename(tmp, cachePath(key)); err != nil {
		_ = os.Remove(tmp)
	}
}

func (s *server) handleAnalyze(w http.ResponseWriter, r *http.Request) {
	r.Body = http.MaxBytesReader(w, r.Body, maxBodyBytes)

	var req models.AnalyzeRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		sendError(w, "Invalid JSON payload", http.StatusBadRequest)
		return
	}

	repoURL, err := git.NormalizeRepoURL(req.RepoURL)
	if err != nil {
		sendError(w, err.Error(), http.StatusBadRequest)
		return
	}

	userID, _ := r.Context().Value(userIDKey).(string)
	logger.Log(logger.InfoLevel, "Analyze request from %s: %s", userID, repoURL)

	commit, err := git.GetRemoteCommitHash(repoURL)
	if err != nil {
		logger.Log(logger.ErrorLevel, "ls-remote failed for %s: %v", repoURL, err)
		sendError(w, "Could not reach that repository. Is it public?", http.StatusBadRequest)
		return
	}

	level := req.RefreshLevel()
	key := cacheKey(repoURL, commit)

	if level == models.RefreshNone {
		if city := readCachedCity(key); city != nil {
			logger.Log(logger.InfoLevel, "Serving cached city for %s@%s", repoURL, commit[:8])
			writeJSON(w, http.StatusOK, models.AnalyzeResponse{Status: "success", CityData: city})
			return
		}
	} else {
		logger.Log(logger.InfoLevel, "Regenerating %s@%s at level %q", repoURL, commit[:8], level)
	}

	// Single-flight is keyed by the refresh level as well as the commit, so a
	// forced regeneration never quietly attaches to a weaker run that is
	// already in flight and would not redo the work being asked for.
	jobKey := key
	if level != models.RefreshNone {
		jobKey = key + "|" + level
	}

	jobID := uuid.New().String()
	job, started := s.jobs.GetOrStart(jobKey, jobID, repoURL, commit, func(j *jobs.Job) {
		s.runAnalysis(j, key, level)
	})
	if !started {
		logger.Log(logger.InfoLevel, "Attaching to in-flight analysis of %s", repoURL)
	}

	writeJSON(w, http.StatusAccepted, models.AnalyzeResponse{Status: "pending", JobID: job.ID})
}

// runAnalysis performs the two-pass pipeline on a detached context, so the
// work survives the originating HTTP request.
func (s *server) runAnalysis(j *jobs.Job, key, level string) {
	ctx, cancel := context.WithTimeout(context.Background(), analysisLimit)
	defer cancel()

	fail := func(msg string, err error) {
		logger.Log(logger.ErrorLevel, "%s: %v", msg, err)
		j.Emit(jobs.Event{Type: jobs.EventError, Error: msg})
	}

	j.Stage(pipeline.Stage{Name: "cloning"})

	// A checkout is content-addressed by commit, so any existing clone of this
	// commit is reusable regardless of who requested it. Scoping clones per
	// user only produced redundant copies of the same tree.
	clonePath := ""
	if id, path, found := db.FindClone(j.RepoURL, j.Commit); found {
		if models.Redoes(level, models.RefreshClone) {
			logger.Log(logger.InfoLevel, "Discarding checkout %s before re-cloning", path)
			if strings.Contains(path, "repolis-session-") {
				_ = os.RemoveAll(path)
			}
			db.DeleteSession(id)
		} else if _, statErr := os.Stat(path); statErr == nil {
			clonePath = path
		}
	}
	if clonePath == "" {
		sessionID := uuid.New().String()
		path, err := git.CloneRepo(j.RepoURL, sessionID)
		if err != nil {
			fail("Failed to clone repository", err)
			return
		}
		clonePath = path
		if err := db.CreateSession(sessionID, "shared", j.RepoURL, clonePath, j.Commit); err != nil {
			logger.Log(logger.WarnLevel, "Could not record session: %v", err)
		}
	}

	state, draft, err := pipeline.Extract(clonePath, j.Stage)
	if err != nil {
		fail("Failed to analyse repository", err)
		return
	}

	// Publish the deterministic city immediately: it is complete, navigable,
	// and available in a fraction of the time the LLM pass takes.
	j.Emit(jobs.Event{Type: jobs.EventCity, City: draft, Refined: false})
	writeCachedCity(key, draft)
	logger.Log(logger.InfoLevel, "Draft city published: %d districts, %d buildings, %d edges",
		len(draft.Districts), draft.Stats.TotalBuildings, len(draft.Dependencies))

	client, err := llm.NewClient(s.cache)
	if err != nil {
		logger.Log(logger.WarnLevel, "LLM unavailable (%v); serving the deterministic city", err)
		j.Emit(jobs.Event{Type: jobs.EventDone})
		return
	}
	if models.Redoes(level, models.RefreshModel) {
		// Ask everything again. Answers are still written back, so the cache
		// ends up refreshed rather than bypassed permanently.
		client.SetCacheBypass(true)
		logger.Log(logger.InfoLevel, "Ignoring cached model answers for this run")
	}

	final := pipeline.Refine(ctx, state, client, j.Stage)
	writeCachedCity(key, final)
	j.Emit(jobs.Event{Type: jobs.EventCity, City: final, Refined: true})
	j.Emit(jobs.Event{Type: jobs.EventDone})

	logger.Log(logger.InfoLevel, "Refined city: %d LLM calls (%d cached), extract %dms, semantic %dms",
		final.Stats.LLMCalls, final.Stats.LLMCacheHits, final.Stats.ExtractMillis, final.Stats.SemanticMillis)
}

func (s *server) handleJobEvents(w http.ResponseWriter, r *http.Request) {
	job, ok := s.jobs.ByID(r.PathValue("id"))
	if !ok {
		sendError(w, "Unknown job", http.StatusNotFound)
		return
	}

	flusher, ok := w.(http.Flusher)
	if !ok {
		sendError(w, "Streaming unsupported", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")
	w.Header().Set("X-Accel-Buffering", "no")
	w.WriteHeader(http.StatusOK)
	flusher.Flush()

	events, unsubscribe := job.Subscribe()
	defer unsubscribe()

	keepalive := time.NewTicker(15 * time.Second)
	defer keepalive.Stop()

	for {
		select {
		case <-r.Context().Done():
			return
		case <-keepalive.C:
			fmt.Fprint(w, ": keepalive\n\n")
			flusher.Flush()
		case e, open := <-events:
			if !open {
				return
			}
			frame, err := jobs.MarshalSSE(e)
			if err != nil {
				continue
			}
			if _, err := w.Write(frame); err != nil {
				return
			}
			flusher.Flush()
			if e.Type == jobs.EventDone || e.Type == jobs.EventError {
				return
			}
		}
	}
}

// handleExplain produces a description of one building on demand. This is the
// only place the large model runs, and nothing waits on it.
func (s *server) handleExplain(w http.ResponseWriter, r *http.Request) {
	r.Body = http.MaxBytesReader(w, r.Body, maxBodyBytes)

	var req models.ExplainRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		sendError(w, "Invalid JSON payload", http.StatusBadRequest)
		return
	}
	repoURL, err := git.NormalizeRepoURL(req.RepoURL)
	if err != nil {
		sendError(w, err.Error(), http.StatusBadRequest)
		return
	}
	if req.BuildingID == "" {
		sendError(w, "building_id is required", http.StatusBadRequest)
		return
	}

	commit, err := git.GetRemoteCommitHash(repoURL)
	if err != nil {
		sendError(w, "Could not reach that repository", http.StatusBadRequest)
		return
	}

	if text, ok := db.GetExplanation(commit, req.BuildingID); ok {
		writeJSON(w, http.StatusOK, models.ExplainResponse{
			Status: "success", BuildingID: req.BuildingID, Explanation: text,
		})
		return
	}

	city := readCachedCity(cacheKey(repoURL, commit))
	if city == nil {
		sendError(w, "City not analysed yet", http.StatusConflict)
		return
	}

	var building *models.Building
	for di := range city.Districts {
		for bi := range city.Districts[di].Buildings {
			if city.Districts[di].Buildings[bi].ID == req.BuildingID {
				building = &city.Districts[di].Buildings[bi]
			}
		}
	}
	if building == nil {
		sendError(w, "Unknown building", http.StatusNotFound)
		return
	}

	var callers, callees []string
	nameOf := func(id string) string {
		if i := strings.LastIndex(id, "::"); i != -1 {
			return id[i+2:]
		}
		return id
	}
	for _, e := range city.Dependencies {
		if e.Source == req.BuildingID && len(callees) < 12 {
			callees = append(callees, nameOf(e.Target))
		}
		if e.Target == req.BuildingID && len(callers) < 12 {
			callers = append(callers, nameOf(e.Source))
		}
	}

	source := ""
	if _, clonePath, ok := db.FindClone(repoURL, commit); ok {
		source = analyzer.ExtractSymbolSource(
			filepath.Join(clonePath, building.SourceFile),
			append([]string{building.Name}, building.Methods...),
		)
	}

	client, err := llm.NewClient(s.cache)
	if err != nil {
		sendError(w, "Explanations are unavailable: no LLM configured", http.StatusServiceUnavailable)
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 2*time.Minute)
	defer cancel()

	text, err := client.ExplainBuilding(ctx, *building, source, callers, callees)
	if err != nil {
		logger.Log(logger.WarnLevel, "Explain failed for %s: %v", req.BuildingID, err)
		sendError(w, "Could not generate an explanation", http.StatusBadGateway)
		return
	}

	db.PutExplanation(commit, req.BuildingID, text)
	writeJSON(w, http.StatusOK, models.ExplainResponse{
		Status: "success", BuildingID: req.BuildingID, Explanation: text,
	})
}

func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

func sendError(w http.ResponseWriter, message string, status int) {
	writeJSON(w, status, models.AnalyzeResponse{Status: "error", Error: message})
}
