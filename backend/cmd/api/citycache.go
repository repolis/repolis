// The on-disk city cache: one JSON file per repository and commit.

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
const cacheSchema = "v2"

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

// writeCachedCity writes atomically: a plain WriteFile can be read
// half-written by a concurrent request.
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
