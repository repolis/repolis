package util

func Clamp(v, lo, hi int) int {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

// Deliberately shares a name with geom.Area, to prove that resolution follows
// imports rather than bare names.
func Area(x float64) float64 {
	return x
}
