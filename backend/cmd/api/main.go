package main

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"github.com/repolis/repolis/backend/internal/logger"

	"github.com/google/uuid"
	"github.com/joho/godotenv"
	"github.com/repolis/repolis/backend/internal/analyzer"
	"github.com/repolis/repolis/backend/internal/db"
	"github.com/repolis/repolis/backend/internal/git"
	"github.com/repolis/repolis/backend/internal/llm"
	"github.com/repolis/repolis/backend/internal/models"
)

type contextKey string

const userIDKey contextKey = "userID"

func CookieMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		cookie, err := r.Cookie("repolis_user_id")
		var userID string
		if err != nil {
			userID = uuid.New().String()
			isProd := os.Getenv("APP_ENV") == "production"

			http.SetCookie(w, &http.Cookie{
				Name:     "repolis_user_id",
				Value:    userID,
				Path:     "/",
				HttpOnly: true,
				Secure:   isProd,
				SameSite: http.SameSiteLaxMode,
				Expires:  time.Now().Add(365 * 24 * time.Hour),
			})
		} else {
			userID = cookie.Value
		}

		ctx := context.WithValue(r.Context(), userIDKey, userID)
		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

func main() {
	_ = godotenv.Load()

	if err := db.InitDB(); err != nil {
		logger.Log(logger.FatalLevel, "Failed to initialize database: %v", err)
	}

	if err := os.MkdirAll(filepath.Join("data", "cities"), 0755); err != nil {
		logger.Log(logger.FatalLevel, "Failed to create cache directory: %v", err)
	}

	mux := http.NewServeMux()

	mux.HandleFunc("POST /api/analyze", handleAnalyze)

	logger.Log(logger.InfoLevel, "Backend server is running on http://localhost:8080")
	if err := http.ListenAndServe(":8080", CookieMiddleware(mux)); err != nil {
		logger.Log(logger.FatalLevel, "Server crashed: %v", err)
	}
}

func handleAnalyze(w http.ResponseWriter, r *http.Request) {
	var req models.AnalyzeRequest

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		sendJSONError(w, "Invalid JSON payload", http.StatusBadRequest)
		return
	}

	if req.RepoURL == "" {
		sendJSONError(w, "repo_url is required", http.StatusBadRequest)
		return
	}

	userID, ok := r.Context().Value(userIDKey).(string)
	if !ok {
		sendJSONError(w, "Internal server error: missing user ID", http.StatusInternalServerError)
		return
	}

	logger.Log(logger.InfoLevel, "Received request from %s to analyze: %s", userID, req.RepoURL)

	remoteCommit, err := git.GetRemoteCommitHash(req.RepoURL)
	if err != nil {
		logger.Log(logger.ErrorLevel, "Failed to fetch remote commit: %v", err)
		sendJSONError(w, "Failed to fetch remote repository info. Ensure it is public.", http.StatusBadRequest)
		return
	}

	cacheKey := fmt.Sprintf("%x", sha256.Sum256([]byte(req.RepoURL+"@"+remoteCommit)))
	cacheFilePath := filepath.Join("data", "cities", cacheKey+".json")

	if !req.Force {
		if b, err := os.ReadFile(cacheFilePath); err == nil {
			var cachedCity models.CityMap
			if json.Unmarshal(b, &cachedCity) == nil {
				logger.Log(logger.InfoLevel, "Serving cached city map for %s (commit: %s)", req.RepoURL, remoteCommit)
				resp := models.AnalyzeResponse{
					Status:   "success",
					CityData: &cachedCity,
				}
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusOK)
				json.NewEncoder(w).Encode(resp)
				return
			}
		}
	} else {
		logger.Log(logger.InfoLevel, "Force flag provided, bypassing cache for %s", req.RepoURL)
	}

	var finalSessionID string
	var finalClonePath string

	if existingSessionID, existingCommit, existingClonePath, err := db.GetSessionByUserAndRepo(userID, req.RepoURL); err == nil && existingSessionID != "" {
		if existingCommit == remoteCommit {
			if _, statErr := os.Stat(existingClonePath); statErr == nil {
				logger.Log(logger.InfoLevel, "Found existing session %s with matching commit %s. Skipping clone.", existingSessionID, existingCommit)
				finalSessionID = existingSessionID
				finalClonePath = existingClonePath
			} else {
				logger.Log(logger.InfoLevel, "Cached clone path gone (%s). Will re-clone.", existingClonePath)
			}
		} else {
			logger.Log(logger.InfoLevel, "Remote repo has updated. Will clone anew.")
		}
	}

	if finalClonePath == "" {
		finalSessionID = uuid.New().String()
		clonePath, err := git.CloneRepo(req.RepoURL, finalSessionID)
		if err != nil {
			sendJSONError(w, "Failed to clone repository", http.StatusInternalServerError)
			return
		}

		if err := db.CreateSession(finalSessionID, userID, req.RepoURL, clonePath, remoteCommit); err != nil {
			sendJSONError(w, "Failed to save session to database", http.StatusInternalServerError)
			return
		}
		finalClonePath = clonePath
	}

	logger.Log(logger.InfoLevel, "Extracting AST structure from: %s", finalClonePath)
	rawData, err := analyzer.ExtractRepository(finalClonePath)
	if err != nil {
		logger.Log(logger.ErrorLevel, "AST extraction failed: %v", err)
		sendJSONError(w, "Failed to analyze repository AST", http.StatusInternalServerError)
		return
	}

	logger.Log(logger.InfoLevel, "Building city via LLM pipeline")
	llmClient, err := llm.NewClient()
	if err != nil {
		sendJSONError(w, "Failed to create LLM client", http.StatusInternalServerError)
		logger.Log(logger.ErrorLevel, "Failed to create LLM client: %v", err)
		return
	}

	cityMap, err := llmClient.BuildCity(r.Context(), finalClonePath, rawData)
	if err != nil {
		sendJSONError(w, "Failed to build city map", http.StatusInternalServerError)
		logger.Log(logger.ErrorLevel, "LLM city build failed: %v", err)
		return
	}

	if b, err := json.Marshal(cityMap); err == nil {
		if err := os.WriteFile(cacheFilePath, b, 0644); err != nil {
			logger.Log(logger.WarnLevel, "Failed to write cache file: %v", err)
		} else {
			logger.Log(logger.InfoLevel, "Saved generated city map to cache: %s", cacheFilePath)
		}
	}

	resp := models.AnalyzeResponse{
		Status:   "success",
		CityData: cityMap,
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(resp)
}

func sendJSONError(w http.ResponseWriter, message string, statusCode int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	json.NewEncoder(w).Encode(models.AnalyzeResponse{
		Status: "error",
		Error:  message,
	})
}
