package git

import (
	"bufio"
	"os/exec"
	"sort"
	"strings"
	"time"
)

// FileHistory is the aggregated git history for one path.
type FileHistory struct {
	Churn        int // number of commits touching the file
	LastModified time.Time
	// The oldest commit touching the file: when this part came into being.
	// Free, since `git log` is already walked to its end.
	FirstSeen     time.Time
	PrimaryAuthor string
}

const (
	recSep   = "\x1e" // between commits
	fieldSep = "\x1f" // between fields of the commit header
)

// ExtractHistory walks the whole history exactly once: O(commits) rather than
// the O(files x commits) of a `git log` per file.
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

		if line == "" {
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

	for path, counts := range authorCounts {
		type ac struct {
			name string
			n    int
		}
		list := make([]ac, 0, len(counts))
		for name, n := range counts {
			list = append(list, ac{name, n})
		}
		// By count then name, for determinism.
		sort.Slice(list, func(i, j int) bool {
			if list[i].n != list[j].n {
				return list[i].n > list[j].n
			}
			return list[i].name < list[j].name
		})
		if len(list) > 0 {
			history[path].PrimaryAuthor = list[0].name
		}
	}

	return history, nil
}
