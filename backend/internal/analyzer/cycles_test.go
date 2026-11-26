package analyzer

import (
	"reflect"
	"testing"

	"github.com/repolis/repolis/backend/internal/models"
)

func loopsOf(t *testing.T, ids []string, edges [][2]string) [][]string {
	t.Helper()
	buildings := make([]models.Building, len(ids))
	for i, id := range ids {
		buildings[i] = models.Building{ID: id, Namespace: id}
	}
	deps := make([]models.DependencyEdge, len(edges))
	for i, e := range edges {
		deps[i] = models.DependencyEdge{Source: e[0], Target: e[1], Weight: 1}
	}
	cycles := FindCycles(buildings, deps)
	if len(cycles) != 1 {
		t.Fatalf("got %d cycles, want 1", len(cycles))
	}
	return cycles[0].Loops
}

func TestLoopReadsInCallOrder(t *testing.T) {
	got := loopsOf(t, []string{"a", "b", "c"}, [][2]string{{"b", "c"}, {"c", "a"}, {"a", "b"}})
	want := [][]string{{"a", "b", "c"}}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("loops = %v, want %v", got, want)
	}
}

func TestTangleSplitsIntoShortestLoops(t *testing.T) {
	got := loopsOf(t, []string{"a", "b", "c"}, [][2]string{{"a", "b"}, {"b", "a"}, {"b", "c"}, {"c", "b"}})
	want := [][]string{{"a", "b"}, {"b", "c"}}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("loops = %v, want %v", got, want)
	}
}
