package db

import (
	"database/sql"
	"os"
	"path/filepath"
	"time"

	_ "modernc.org/sqlite"
)

var DB *sql.DB

func InitDB() error {
	dbPath := os.Getenv("DB_PATH")
	if dbPath == "" {
		dbPath = "./data/sqlite.db"
	}

	if err := os.MkdirAll(filepath.Dir(dbPath), 0755); err != nil {
		return err
	}

	var err error
	DB, err = sql.Open("sqlite", dbPath)
	if err != nil {
		return err
	}

	createTableQuery := `
	CREATE TABLE IF NOT EXISTS sessions (
		id TEXT PRIMARY KEY,
		user_id TEXT NOT NULL,
		repo_url TEXT NOT NULL,
		clone_path TEXT NOT NULL,
		commit_hash TEXT NOT NULL DEFAULT '',
		created_at DATETIME DEFAULT CURRENT_TIMESTAMP
	);
	`
	_, err = DB.Exec(createTableQuery)
	if err == nil {
		_, _ = DB.Exec(`ALTER TABLE sessions ADD COLUMN commit_hash TEXT DEFAULT ''`)
	}
	return err
}

func CreateSession(sessionID, userID, repoURL, clonePath, commitHash string) error {
	query := `INSERT INTO sessions (id, user_id, repo_url, clone_path, commit_hash, created_at) VALUES (?, ?, ?, ?, ?, ?)`
	_, err := DB.Exec(query, sessionID, userID, repoURL, clonePath, commitHash, time.Now())
	return err
}

func GetSessionByUserAndRepo(userID, repoURL string) (string, string, string, error) {
	var sessionID, commitHash, clonePath string
	query := `SELECT id, commit_hash, clone_path FROM sessions WHERE user_id = ? AND repo_url = ? ORDER BY created_at DESC LIMIT 1`
	err := DB.QueryRow(query, userID, repoURL).Scan(&sessionID, &commitHash, &clonePath)
	if err != nil {
		return "", "", "", err
	}
	return sessionID, commitHash, clonePath, nil
}
