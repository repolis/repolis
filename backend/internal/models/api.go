package models

type AnalyzeRequest struct {
	RepoURL string `json:"repo_url"`
	Force   bool   `json:"force"`
}

type AnalyzeResponse struct {
	Status   string   `json:"status"`
	CityData *CityMap `json:"cityData,omitempty"`
	Error    string   `json:"error,omitempty"`
}
