package main

import (
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/joho/godotenv"

	"github.com/repolis/repolis/backend/internal/db"
	"github.com/repolis/repolis/backend/internal/jobs"
	"github.com/repolis/repolis/backend/internal/llm"
	"github.com/repolis/repolis/backend/internal/logger"
)

const (
	maxBodyBytes  = 64 << 10
	cloneMaxAge   = 24 * time.Hour
	analysisLimit = 20 * time.Minute
)

type server struct {
	jobs  *jobs.Manager
	cache *llm.Cache
}

func main() {
	_ = godotenv.Load()

	if err := db.Init(); err != nil {
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
		Handler:           corsMiddleware(cookieMiddleware(mux)),
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
