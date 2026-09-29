package store

type Store struct {
	data map[string]int
	hits int
}

func (s *Store) Put(k string, v int) {
	s.data[k] = v
	s.hits++
}

func (s *Store) Get(k string) int {
	return s.data[k]
}

// A constructor is a free function in Go, not a method.
func NewStore() *Store {
	return &Store{data: map[string]int{}}
}
