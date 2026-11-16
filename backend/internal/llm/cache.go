package llm

import (
	"database/sql"
	"sync"
)

// Keyed by the hash of the exact inputs, so a re-analysis, a later commit or a
// shared vendored library costs nothing.
type Cache struct {
	db *sql.DB
	mu sync.RWMutex
	// Fronts the database so one run never round-trips twice for a prompt.
	mem map[string]string
}

func NewCache(db *sql.DB) *Cache {
	return &Cache{db: db, mem: make(map[string]string)}
}

func (c *Cache) Get(key string) (string, bool) {
	if c == nil {
		return "", false
	}
	c.mu.RLock()
	v, ok := c.mem[key]
	c.mu.RUnlock()
	if ok {
		return v, true
	}
	if c.db == nil {
		return "", false
	}

	var out string
	err := c.db.QueryRow(`SELECT value FROM llm_cache WHERE key = ?`, key).Scan(&out)
	if err != nil {
		return "", false
	}
	c.mu.Lock()
	c.mem[key] = out
	c.mu.Unlock()
	return out, true
}

func (c *Cache) Put(key, value string) {
	if c == nil {
		return
	}
	c.mu.Lock()
	c.mem[key] = value
	c.mu.Unlock()
	if c.db == nil {
		return
	}
	_, _ = c.db.Exec(
		`INSERT INTO llm_cache (key, value, created_at) VALUES (?, ?, CURRENT_TIMESTAMP)
		 ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
		key, value,
	)
}
