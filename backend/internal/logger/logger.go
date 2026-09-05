package logger

import (
	"fmt"
	"log"
	"strings"
)

type Level int

const (
	InfoLevel Level = iota
	WarnLevel
	ErrorLevel
	FatalLevel
)

func Log(level Level, format string, args ...any) {
	var prefix string
	switch level {
	case InfoLevel:
		prefix = "[LOG]"
	case WarnLevel:
		prefix = "[WARNING]"
	case ErrorLevel, FatalLevel:
		prefix = "[ERROR]"
	default:
		prefix = "[LOG]"
	}

	msg := fmt.Sprintf(format, args...)
	msg = strings.TrimSuffix(msg, "\n")

	switch level {
	case FatalLevel:
		log.Fatalf("%s %s", prefix, msg)
	case ErrorLevel:
		log.Printf("%s %s", prefix, msg)
	default:
		fmt.Printf("%s %s\n", prefix, msg)
	}
}
