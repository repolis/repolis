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
	// FirstSeen is the oldest commit that touched the file, i.e. when this
	// part of the system came into existence. It costs nothing extra: the log
	// is already being walked, and `git log` ends at the first commit.
	FirstSeen     time.Time
	PrimaryAuthor string
}

const (
	recSep   = "\x1e" // between commits
	fieldSep = "\x1f" // between fields of the commit header
)

// ExtractHistory walks the whole repository history exactly once.
//
// The previous implementation ran three `git` subprocesses *per file*
// (`rev-list --count`, `log -1 --format=%cI`, `log --format=%an`), each of
// which re-walked the entire history. For a repo with ~1000 source files that
// is ~3000 processes and ~3000 full history traversals, and it was the single
// largest cost in extraction after the LLM. This is O(commits) instead of
// O(files x commits).
func ExtractHistory(repoPath string) (map[string]*FileHistory, error) {
	// --name-only rather than --numstat: line counts would require the blob
	// contents of every revision, which in a blobless partial clone means a
	// network fetch per commit (measured: 9.8s on a 7-file repo). Tree diffs
	// are already local, so churn and recency cost nothing.
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
		// git log walks newest-first, so the first sighting is the latest
		// commit and the last one seen is the oldest.
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
		// Sort by count then name so the result is deterministic.
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
