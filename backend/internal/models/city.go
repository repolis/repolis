package models

import "time"

type FileMetrics struct {
	Path           string    `json:"path"`
	Extension      string    `json:"extension"`
	Depth          int       `json:"depth"`
	LinesOfCode    int       `json:"lines_of_code"`
	FunctionCount  int       `json:"function_count"`
	StructCount    int       `json:"struct_count"`
	TodoCount      int       `json:"todo_count"`
	CommitChurn    int       `json:"commit_churn"`
	LastModified   time.Time `json:"last_modified"`
	PrimaryAuthor  string    `json:"primary_author"`
	Includes       []string  `json:"includes"`
	FunctionNames  []string  `json:"function_names"`
	StructNames    []string  `json:"struct_names"`
	StringLiterals []string  `json:"string_literals"`
	Typology       string    `json:"typology"`
}

type CityMap struct {
	Files []FileMetrics `json:"files"`
}
