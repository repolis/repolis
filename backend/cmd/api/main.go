package main

import (
	"context"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/google/uuid"
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

// CORSMiddleware lets the frontend run on a different origin. A no-op behind
// Vite's dev proxy, required for any deployment that does not proxy.
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
