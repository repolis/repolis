package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/repolis/repolis/backend/internal/models"
)

// cacheSchema is bumped whenever the CityMap shape changes, so a stale cache
// entry can never be served to a renderer that expects different fields.
const cacheSchema = "v3"

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

func writeCachedCity(key string, city *models.CityMap) {
	if b, err := json.Marshal(city); err == nil {
		writeAtomic(cachePath(key), b)
	}
}

func lastCommitPath(repoURL string) string {
	return filepath.Join("data", "cities", fmt.Sprintf("%x", sha256.Sum256([]byte(repoURL)))+".last")
}

func rememberCommit(repoURL, commit string) {
	writeAtomic(lastCommitPath(repoURL), []byte(commit))
}

func lastCachedCommit(repoURL string) (string, bool) {
	b, err := os.ReadFile(lastCommitPath(repoURL))
	if err != nil || len(b) == 0 {
		return "", false
	}
	return string(b), true
}

func writeAtomic(path string, b []byte) {
	tmp := path + ".tmp"
	if os.WriteFile(tmp, b, 0o644) != nil {
		return
	}
	if err := os.Rename(tmp, path); err != nil {
		_ = os.Remove(tmp)
	}
}
