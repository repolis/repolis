package models

import "testing"

func TestRefreshLevel(t *testing.T) {
	cases := []struct {
		req  AnalyzeRequest
		want string
	}{
		{AnalyzeRequest{}, RefreshNone},
		{AnalyzeRequest{Refresh: "city"}, RefreshCity},
		{AnalyzeRequest{Refresh: "model"}, RefreshModel},
		{AnalyzeRequest{Refresh: "clone"}, RefreshClone},
		// Older clients only know the boolean.
		{AnalyzeRequest{Force: true}, RefreshCity},
		// A stronger explicit level wins over the legacy flag.
		{AnalyzeRequest{Refresh: "model", Force: true}, RefreshModel},
		// Anything unrecognised must not error, just not refresh.
		{AnalyzeRequest{Refresh: "everything"}, RefreshNone},
		{AnalyzeRequest{Refresh: "CLONE"}, RefreshNone},
	}
	for _, c := range cases {
		if got := c.req.RefreshLevel(); got != c.want {
			t.Errorf("%+v -> %q, want %q", c.req, got, c.want)
		}
	}
}

func TestRedoes(t *testing.T) {
	if !Redoes(RefreshClone, RefreshModel) {
		t.Error("clone should imply model")
	}
	if !Redoes(RefreshModel, RefreshCity) {
		t.Error("model should imply city")
	}
	if Redoes(RefreshCity, RefreshModel) {
		t.Error("city must not imply model")
	}
	if Redoes(RefreshNone, RefreshCity) {
		t.Error("none must not imply city")
	}
}
