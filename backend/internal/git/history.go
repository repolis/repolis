package git

import (
	"bufio"
	"os/exec"
	"strings"
	"time"
)

type FileHistory struct {
	Churn        int // number of commits touching the file
	LastModified time.Time
	// Free, since `git log` is already walked to its end.
	FirstSeen     time.Time
	PrimaryAuthor string
}

const (
	recSep   = "\x1e" // between commits
	fieldSep = "\x1f" // between fields of the commit header
)

// One pass over the whole history: O(commits) rather than a `git log` per file.
func ExtractHistory(repoPath string) (map[string]*FileHistory, error) {
	// --name-only, not --numstat: line counts need every revision's blobs,
	// which in a blobless clone means a fetch per commit (9.8s on 7 files).
	// Tree diffs are already local.
	cmd := exec.Command("git", "log",
		"--no-merges",
		"--name-only",
		"--format="+recSep+"%H"+fieldSep+"%cI"+fieldSep+"%an",
		"--no-renames",
	)
	cmd.Dir = repoPath

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}
	if err := cmd.Start(); err != nil {
		return nil, err
	}

	history := make(map[string]*FileHistory)
	authorCounts := make(map[string]map[string]int)

	var when time.Time
	var author string

	scanner := bufio.NewScanner(stdout)
	scanner.Buffer(make([]byte, 0, 256*1024), 4*1024*1024)

	for scanner.Scan() {
		line := scanner.Text()

		if strings.HasPrefix(line, recSep) {
			parts := strings.Split(strings.TrimPrefix(line, recSep), fieldSep)
			if len(parts) >= 3 {
				when, _ = time.Parse(time.RFC3339, parts[1])
				author = parts[2]
			}
			continue
		}

		path := strings.TrimSpace(line)
		if path == "" {
			continue
		}

		h := history[path]
		if h == nil {
			h = &FileHistory{}
			history[path] = h
			authorCounts[path] = make(map[string]int)
		}
		h.Churn++
		// Newest-first, so the first sighting is the latest commit.
		if h.LastModified.IsZero() && !when.IsZero() {
			h.LastModified = when
		}
		if !when.IsZero() {
			h.FirstSeen = when
		}
		if author != "" {
			authorCounts[path][author]++
		}
	}

	_ = cmd.Wait()
	if err := scanner.Err(); err != nil {
		return history, err
	}

	// Most commits wins; the name breaks ties, for determinism.
	for path, counts := range authorCounts {
		best, bestN := "", 0
		for name, n := range counts {
			if n > bestN || n == bestN && name < best {
				best, bestN = name, n
			}
		}
		history[path].PrimaryAuthor = best
	}

	return history, nil
}
