package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"

	"github.com/repolis/repolis/backend/internal/models"
)

func main() {
	mux := http.NewServeMux()

	mux.HandleFunc("POST /api/analyze", handleAnalyze)

	fmt.Println("Backend server is running on http://localhost:8080")
	if err := http.ListenAndServe(":8080", mux); err != nil {
		log.Fatalf("Server crashed: %v", err)
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

	fmt.Printf("Received request to analyze: %s\n", req.RepoURL)

	// TODO:
	// 1. Pass req.RepoURL to internal/git to clone the repo
	// 2. Pass the cloned path to internal/analyzer to run tree-sitter/LLM
	// 3. Construct the real CityData

	resp := models.AnalyzeResponse{
		Status: "success",
		CityData: map[string]string{
			"message":  "Building successfully connected!",
			"repo":     req.RepoURL,
			"mockData": "This will be real AST data soon.",
		},
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
