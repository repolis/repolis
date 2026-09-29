package db

import (
	"database/sql"
	"os"
	"path/filepath"
	"time"

	_ "modernc.org/sqlite"
)

var DB *sql.DB

const schema = `
CREATE TABLE IF NOT EXISTS sessions (
	id          TEXT PRIMARY KEY,
	user_id     TEXT NOT NULL,
	repo_url    TEXT NOT NULL,
	clone_path  TEXT NOT NULL,
	commit_hash TEXT NOT NULL DEFAULT '',
	created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_sessions_repo ON sessions(repo_url, created_at DESC);

-- Every LLM decision, keyed by the hash of its exact prompt inputs.
CREATE TABLE IF NOT EXISTS llm_cache (
	key        TEXT PRIMARY KEY,
	value      TEXT NOT NULL,
	created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- On-demand building explanations, keyed by commit so they stay correct.
CREATE TABLE IF NOT EXISTS explanations (
	commit_hash TEXT NOT NULL,
	building_id TEXT NOT NULL,
	text        TEXT NOT NULL,
	created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
	PRIMARY KEY (commit_hash, building_id)
);
`

func InitDB() error {
	dbPath := os.Getenv("DB_PATH")
	if dbPath == "" {
		dbPath = "./data/sqlite.db"
	}
	if err := os.MkdirAll(filepath.Dir(dbPath), 0o755); err != nil {
		return err
	}

	var err error
	DB, err = sql.Open("sqlite", dbPath+"?_pragma=busy_timeout(5000)&_pragma=journal_mode(WAL)")
	if err != nil {
		return err
	}
	DB.SetMaxOpenConns(1) // modernc/sqlite is happiest serialised
	_, err = DB.Exec(schema)
	return err
}

func CreateSession(sessionID, userID, repoURL, clonePath, commitHash string) error {
	_, err := DB.Exec(
		`INSERT INTO sessions (id, user_id, repo_url, clone_path, commit_hash, created_at)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		sessionID, userID, repoURL, clonePath, commitHash, time.Now(),
	)
	return err
}

// FindClone returns any existing checkout of a repo at a commit, whoever made
// it: a clone is immutable and addressed by commit.
func FindClone(repoURL, commitHash string) (string, string, bool) {
	var id, path string
	err := DB.QueryRow(
		`SELECT id, clone_path FROM sessions
		 WHERE repo_url = ? AND commit_hash = ?
		 ORDER BY created_at DESC LIMIT 1`,
		repoURL, commitHash,
	).Scan(&id, &path)
	if err != nil {
		return "", "", false
	}
	return id, path, true
}

// StaleClones lists checkouts past the cutoff, so an analysis does not leak
// its temp directory forever.
func StaleClones(olderThan time.Duration) map[string]string {
	out := make(map[string]string)
	rows, err := DB.Query(
		`SELECT id, clone_path FROM sessions WHERE created_at < ?`,
		time.Now().Add(-olderThan),
	)
	if err != nil {
		return out
	}
	defer rows.Close()
	for rows.Next() {
		var id, path string
		if rows.Scan(&id, &path) == nil {
			out[id] = path
		}
	}
	return out
}

func DeleteSession(id string) {
	_, _ = DB.Exec(`DELETE FROM sessions WHERE id = ?`, id)
}

func GetExplanation(commitHash, buildingID string) (string, bool) {
	var text string
	err := DB.QueryRow(
		`SELECT text FROM explanations WHERE commit_hash = ? AND building_id = ?`,
		commitHash, buildingID,
	).Scan(&text)
	return text, err == nil
}

func PutExplanation(commitHash, buildingID, text string) {
	_, _ = DB.Exec(
		`INSERT INTO explanations (commit_hash, building_id, text) VALUES (?, ?, ?)
		 ON CONFLICT(commit_hash, building_id) DO UPDATE SET text = excluded.text`,
		commitHash, buildingID, text,
	)
}
