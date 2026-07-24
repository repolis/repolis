package models

type AnalyzeRequest struct {
	RepoURL string `json:"repo_url"`
}

type AnalyzeResponse struct {
	Status   string   `json:"status"`
	CityData *CityMap `json:"cityData,omitempty"`
	Error    string   `json:"error,omitempty"`
}
