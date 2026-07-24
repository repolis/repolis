package main

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"time"

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
			http.SetCookie(w, &http.Cookie{
				Name:     "repolis_user_id",
				Value:    userID,
				Path:     "/",
				HttpOnly: true,
				Secure:   false, // set to true in prod
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
	_ = godotenv.Load() // Load .env file if present

	if err := db.InitDB(); err != nil {
		log.Fatalf("[ERROR] Failed to initialize database: %v", err)
	}

	mux := http.NewServeMux()

	mux.HandleFunc("POST /api/analyze", handleAnalyze)

	fmt.Println("[LOG] Backend server is running on http://localhost:8080")
	if err := http.ListenAndServe(":8080", CookieMiddleware(mux)); err != nil {
		log.Fatalf("[ERROR] Server crashed: %v", err)
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

	fmt.Printf("[LOG] Received request from %s to analyze: %s\n", userID, req.RepoURL)

	remoteCommit, err := git.GetRemoteCommitHash(req.RepoURL)
	if err != nil {
		fmt.Printf("[ERROR] Failed to fetch remote commit: %v\n", err)
		sendJSONError(w, "Failed to fetch remote repository info. Ensure it is public.", http.StatusBadRequest)
		return
	}

	var finalSessionID string
	var finalClonePath string

	if existingSessionID, existingCommit, existingClonePath, err := db.GetSessionByUserAndRepo(userID, req.RepoURL); err == nil && existingSessionID != "" {
		if existingCommit == remoteCommit {
			fmt.Printf("[LOG] Found existing session %s with matching commit %s. Skipping clone.\n", existingSessionID, existingCommit)
			finalSessionID = existingSessionID
			finalClonePath = existingClonePath
		} else {
			fmt.Printf("[LOG] Remote repo has updated. Will clone anew.\n")
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

	fmt.Printf("[LOG] Analyzing AST for: %s\n", finalClonePath)
	cityMap, err := analyzer.AnalyzeRepository(finalClonePath)
	if err != nil {
		sendJSONError(w, "Failed to analyze repository AST", http.StatusInternalServerError)
		return
	}

	fmt.Printf("[LOG] Invoking LLM to determine building typologies...\n")
	llmClient, err := llm.NewClient()
	if err != nil {
		sendJSONError(w, "Failed to create LLM client", http.StatusInternalServerError)
		log.Fatalf("[ERROR] Failed to create LLM client: %v", err)
		return
	}
	llmClient.CategorizeCity(r.Context(), finalClonePath, cityMap)

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
