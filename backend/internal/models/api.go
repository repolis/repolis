package models

// Refresh levels control how much work a re-analysis redoes. Each level
// implies the ones above it, and each costs noticeably more than the last.
const (
	// RefreshNone serves the cached city map when one exists.
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

// RefreshLevels lists the accepted values, weakest first.
var RefreshLevels = []string{RefreshNone, RefreshCity, RefreshModel, RefreshClone}

type AnalyzeRequest struct {
	RepoURL string `json:"repo_url"`
	// Refresh is one of the Refresh* constants. Empty means "use the cache".
	Refresh string `json:"refresh,omitempty"`
	// Force is the older boolean spelling, equivalent to Refresh="city".
	Force bool `json:"force,omitempty"`
}

// RefreshLevel normalises the request into exactly one known level. An
// unrecognised value is treated as no refresh rather than rejected, so a
// stale client cannot be made to fail by a value it does not know about.
func (r AnalyzeRequest) RefreshLevel() string {
	for _, l := range RefreshLevels {
		if l != RefreshNone && r.Refresh == l {
			return l
		}
	}
	if r.Force {
		return RefreshCity
	}
	return RefreshNone
}

// Redoes reports whether this level is at least as strong as `level`.
func Redoes(current, level string) bool {
	rank := func(l string) int {
		for i, v := range RefreshLevels {
			if v == l {
				return i
			}
		}
		return 0
	}
	return rank(current) >= rank(level)
}

type AnalyzeResponse struct {
	Status   string   `json:"status"`
	JobID    string   `json:"job_id,omitempty"`
	CityData *CityMap `json:"cityData,omitempty"`
	Error    string   `json:"error,omitempty"`
}

type ExplainRequest struct {
	RepoURL    string `json:"repo_url"`
	BuildingID string `json:"building_id"`
}

type ExplainResponse struct {
	Status      string `json:"status"`
	BuildingID  string `json:"building_id"`
	Explanation string `json:"explanation,omitempty"`
	Error       string `json:"error,omitempty"`
}
