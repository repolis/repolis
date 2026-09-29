// Package jobs runs analyses in the background and streams progress.
//
// The previous handler did everything synchronously on the request context, so
// a client disconnect aborted minutes of work and discarded the partial
// result, and two tabs on the same repository did all the work twice with a
// non-atomic race on the cache file.
package jobs

import (
	"encoding/json"
	"sync"
	"time"

	"github.com/repolis/repolis/backend/internal/models"
	"github.com/repolis/repolis/backend/internal/pipeline"
)

type EventType string

const (
	EventStage EventType = "stage"
	EventCity  EventType = "city"
	EventDone  EventType = "done"
	EventError EventType = "error"
)

type Event struct {
	Type    EventType       `json:"type"`
	Stage   *pipeline.Stage `json:"stage,omitempty"`
	City    *models.CityMap `json:"city,omitempty"`
	Refined bool            `json:"refined,omitempty"`
	Error   string          `json:"error,omitempty"`
}

type Job struct {
	ID      string
	RepoURL string
	Commit  string
	Created time.Time

	mu      sync.Mutex
	history []Event // replayed to late subscribers
	subs    map[chan Event]struct{}
	done    bool
}

func newJob(id, repoURL, commit string) *Job {
	return &Job{
		ID: id, RepoURL: repoURL, Commit: commit, Created: time.Now(),
		subs: make(map[chan Event]struct{}),
	}
}

// Emit records an event and fans it out to every subscriber.
func (j *Job) Emit(e Event) {
	j.mu.Lock()
	// Only the most recent city of each kind is worth replaying; stage events
	// are transient. This keeps the replay buffer small and bounded.
	if e.Type == EventCity {
		filtered := j.history[:0]
		for _, h := range j.history {
			if h.Type == EventCity && h.Refined == e.Refined {
				continue
			}
			filtered = append(filtered, h)
		}
		j.history = filtered
	}
	if e.Type != EventStage {
		j.history = append(j.history, e)
	}
	if e.Type == EventDone || e.Type == EventError {
		j.done = true
	}
	subs := make([]chan Event, 0, len(j.subs))
	for c := range j.subs {
		subs = append(subs, c)
	}
	j.mu.Unlock()

	for _, c := range subs {
		select {
		case c <- e:
		default: // slow consumer: drop rather than stall the pipeline
		}
	}
}

func (j *Job) Stage(s pipeline.Stage) { j.Emit(Event{Type: EventStage, Stage: &s}) }

// Subscribe returns a channel pre-loaded with the events so far, so a client
// that connects late still receives the draft city.
func (j *Job) Subscribe() (<-chan Event, func()) {
	ch := make(chan Event, 32)
	j.mu.Lock()
	replay := append([]Event(nil), j.history...)
	finished := j.done
	if !finished {
		j.subs[ch] = struct{}{}
	}
	j.mu.Unlock()

	go func() {
		for _, e := range replay {
			ch <- e
		}
		if finished {
			close(ch)
		}
	}()

	return ch, func() {
		j.mu.Lock()
		if _, ok := j.subs[ch]; ok {
			delete(j.subs, ch)
		}
		j.mu.Unlock()
	}
}

func (j *Job) Done() bool {
	j.mu.Lock()
	defer j.mu.Unlock()
	return j.done
}

// Manager provides single-flight: concurrent requests for the same repo and
// commit attach to one running analysis instead of starting another.
type Manager struct {
	mu     sync.Mutex
	byKey  map[string]*Job
	byID   map[string]*Job
	maxAge time.Duration
}

func NewManager() *Manager {
	m := &Manager{
		byKey: make(map[string]*Job), byID: make(map[string]*Job),
		maxAge: 30 * time.Minute,
	}
	go m.reap()
	return m
}

// GetOrStart returns the existing job for key, or starts a new one. The second
// return value reports whether this call created it.
func (m *Manager) GetOrStart(key, id, repoURL, commit string, run func(*Job)) (*Job, bool) {
	m.mu.Lock()
	if j, ok := m.byKey[key]; ok && !j.Done() {
		m.mu.Unlock()
		return j, false
	}
	j := newJob(id, repoURL, commit)
	m.byKey[key] = j
	m.byID[id] = j
	m.mu.Unlock()

	go func() {
		defer func() {
			if r := recover(); r != nil {
				j.Emit(Event{Type: EventError, Error: "internal error during analysis"})
			}
		}()
		run(j)
	}()
	return j, true
}

func (m *Manager) ByID(id string) (*Job, bool) {
	m.mu.Lock()
	defer m.mu.Unlock()
	j, ok := m.byID[id]
	return j, ok
}

func (m *Manager) reap() {
	for range time.Tick(5 * time.Minute) {
		cutoff := time.Now().Add(-m.maxAge)
		m.mu.Lock()
		for k, j := range m.byKey {
			if j.Created.Before(cutoff) {
				delete(m.byKey, k)
			}
		}
		for id, j := range m.byID {
			if j.Created.Before(cutoff) {
				delete(m.byID, id)
			}
		}
		m.mu.Unlock()
	}
}

// MarshalSSE renders an event as a single SSE frame. JSON is compact so the
// payload never contains a newline, which would break the framing.
func MarshalSSE(e Event) ([]byte, error) {
	b, err := json.Marshal(e)
	if err != nil {
		return nil, err
	}
	out := make([]byte, 0, len(b)+16)
	out = append(out, "data: "...)
	out = append(out, b...)
	out = append(out, '\n', '\n')
	return out, nil
}
