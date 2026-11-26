package models

import "slices"

// Each level implies the ones above it and costs noticeably more.
const (
	RefreshNone = ""
	// RefreshCity re-runs the pipeline but keeps the checkout and the cached
	// model answers. Use it after changing extraction, clustering or layout.
	RefreshCity = "city"
	// RefreshModel additionally ignores cached model answers, so every
	// adjudication and district name is asked again. Use it after changing a
	// prompt or switching models.
	RefreshModel = "model"
	// RefreshClone additionally deletes the checkout and clones again. Use it
	// when the working copy is stale or damaged.
	RefreshClone = "clone"
)

// Weakest first.
var RefreshLevels = []string{RefreshNone, RefreshCity, RefreshModel, RefreshClone}

type AnalyzeRequest struct {
	RepoURL string `json:"repo_url"`
	// Empty means the latest commit on the default branch.
	Commit string `json:"commit,omitempty"`
	// Empty means "use the cache".
	Refresh string `json:"refresh,omitempty"`
	// Force is the older boolean spelling, equivalent to Refresh="city".
	Force bool `json:"force,omitempty"`
}

// An unrecognised value means no refresh rather than an error, so a stale
// client cannot be made to fail.
func (r AnalyzeRequest) RefreshLevel() string {
	if r.Refresh != RefreshNone && slices.Contains(RefreshLevels, r.Refresh) {
		return r.Refresh
	}
	if r.Force {
		return RefreshCity
	}
	return RefreshNone
}

func Redoes(current, level string) bool {
	// An unknown level ranks as no refresh.
	rank := func(l string) int { return max(slices.Index(RefreshLevels, l), 0) }
	return rank(current) >= rank(level)
}

type AnalyzeResponse struct {
	Status   string   `json:"status"`
	JobID    string   `json:"job_id,omitempty"`
	Commit   string   `json:"commit,omitempty"`
	CityData *CityMap `json:"cityData,omitempty"`
	Error    string   `json:"error,omitempty"`
}

type ExplainRequest struct {
	RepoURL    string `json:"repo_url"`
	BuildingID string `json:"building_id"`
	Commit     string `json:"commit,omitempty"`
}

type ExplainResponse struct {
	Status      string `json:"status"`
	BuildingID  string `json:"building_id"`
	Explanation string `json:"explanation,omitempty"`
	Error       string `json:"error,omitempty"`
}
