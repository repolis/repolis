package main

import (
	"os"
	"strings"
	"time"

	"github.com/repolis/repolis/backend/internal/db"
)

func reapStaleClones() {
	clean := func() {
		for id, path := range db.StaleClones(cloneMaxAge) {
			if path != "" && strings.Contains(path, "repolis-session-") {
				_ = os.RemoveAll(path)
			}
			db.DeleteSession(id)
		}
	}
	clean()
	for range time.Tick(time.Hour) {
		clean()
	}
}
