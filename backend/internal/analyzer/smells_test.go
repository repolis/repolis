package analyzer

import (
	"testing"

	"github.com/repolis/repolis/backend/internal/models"
)

func plainBuilding() models.Building {
	return models.Building{
		ID: "plain", Kind: "type",
		NumFields: 2, NumMethods: 4,
		MaxComplexity: 3, SumComplexity: 10, FanOut: 2,
	}
}

func smellOf(b models.Building) string {
	buildings := []models.Building{b}
	markSmells(buildings)
	return buildings[0].Smell
}

func TestSmells(t *testing.T) {
	god := plainBuilding()
	god.NumMethods, god.SumComplexity, god.FanOut = 20, 90, 9

	godModule := god
	godModule.Kind = "module"

	brain := plainBuilding()
	brain.MaxComplexity = 30

	data := plainBuilding()
	data.NumFields, data.NumMethods = 10, 0

	dataModule := data
	dataModule.Kind = "module"

	cases := []struct {
		name string
		b    models.Building
		want string
	}{
		{"god class", god, SmellGod},
		{"god module", godModule, SmellGod},
		{"very complex function", brain, SmellBrain},
		{"data-only struct", data, SmellData},
		{"module is never data-only", dataModule, ""},
		{"small building", plainBuilding(), ""},
	}
	for _, c := range cases {
		if got := smellOf(c.b); got != c.want {
			t.Errorf("%s: smell = %q, want %q", c.name, got, c.want)
		}
	}
}

func TestSmellThresholdRisesToTheRepoP95(t *testing.T) {
	buildings := make([]models.Building, 20)
	for i := range buildings {
		buildings[i] = plainBuilding()
		buildings[i].MaxComplexity = 40
	}
	subject := plainBuilding()
	subject.MaxComplexity = 30
	buildings = append(buildings, subject)

	markSmells(buildings)
	if got := buildings[len(buildings)-1].Smell; got != "" {
		t.Errorf("complexity 30 in a repo whose p95 is 40 was flagged %q", got)
	}
}
