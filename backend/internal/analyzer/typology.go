package analyzer

import "strings"

// Typologies is the closed set the renderer knows how to colour. It is also
// the exact list handed to the LLM, so the model picks from an enumeration
// rather than inventing a label.
var Typologies = []string{
	"core", "data", "network", "security", "interface",
	"utility", "config", "test", "example", "unknown",
}

var typologySet = func() map[string]bool {
	m := make(map[string]bool, len(Typologies))
	for _, t := range Typologies {
		m[t] = true
	}
	return m
}()

// IsTypology reports whether t is exactly one of the allowed values.
func IsTypology(t string) bool { return typologySet[strings.ToLower(strings.TrimSpace(t))] }

// typologyHints are ordered most-specific first. The previous fuzzy matcher
// checked "core" (via "system"/"main"/"engine") before "test", so a typology
// of "test system" classified as core.
var typologyHints = []struct {
	needles  []string
	typology string
}{
	{[]string{"test", "mock", "stub", "fixture", "spec", "clar"}, "test"},
	{[]string{"example", "sample", "demo", "tutorial"}, "example"},
	{[]string{"config", "settings", "option", "flag", "env"}, "config"},
	{[]string{"crypt", "secur", "auth", "hash", "sign", "cert", "tls", "ssl", "sha", "md5"}, "security"},
	{[]string{"net", "http", "socket", "tcp", "udp", "transport", "remote", "proto", "api", "web", "url"}, "network"},
	{[]string{"db", "database", "store", "storage", "index", "cache", "odb", "sql", "table", "record", "serial", "parse", "json", "xml", "csv", "entit", "model", "object", "state", "tree", "graph", "node"}, "data"},
	{[]string{"ui", "view", "render", "draw", "window", "widget", "gui", "display", "font", "sprite", "frontend", "curses", "editor", "layout", "input", "cursor"}, "interface"},
	{[]string{"util", "helper", "common", "misc", "tool", "str", "buf", "alloc", "mem", "log", "math", "sort"}, "utility"},
	{[]string{"core", "engine", "kernel", "runtime", "vm", "system", "main", "vdbe", "exec"}, "core"},
}

// GuessTypology maps free text (directory names, symbol names, or an LLM's
// answer) onto the closed set.
func GuessTypology(text string) string {
	t := strings.ToLower(strings.TrimSpace(text))
	if typologySet[t] {
		return t
	}
	for _, h := range typologyHints {
		for _, n := range h.needles {
			if strings.Contains(t, n) {
				return h.typology
			}
		}
	}
	return "unknown"
}
