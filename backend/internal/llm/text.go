package llm

import "strings"

func cleanAnswer(s string) string {
	s = stripThinking(s)
	if i := strings.IndexAny(s, "\n\r"); i != -1 {
		s = s[:i]
	}
	return strings.Trim(strings.TrimSpace(s), " .,;:`\"'*")
}

func truncate(s string, n int) string {
	s = strings.TrimSpace(s)
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}

func stripThinking(s string) string {
	for {
		start := strings.Index(s, "<think>")
		if start == -1 {
			break
		}
		end := strings.Index(s, "</think>")
		if end == -1 || end < start {
			s = s[:start]
			break
		}
		s = s[:start] + s[end+len("</think>"):]
	}
	return strings.TrimSpace(s)
}
