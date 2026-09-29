package logger

import (
	"fmt"
	"log"
	"os"
	"strings"
	"sync/atomic"
)

type Level int

const (
	DebugLevel Level = iota
	InfoLevel
	WarnLevel
	ErrorLevel
	FatalLevel
)

var minLevel atomic.Int32

func init() {
	switch strings.ToLower(os.Getenv("LOG_LEVEL")) {
	case "debug":
		minLevel.Store(int32(DebugLevel))
	case "warn":
		minLevel.Store(int32(WarnLevel))
	case "error":
		minLevel.Store(int32(ErrorLevel))
	default:
		minLevel.Store(int32(InfoLevel))
	}
}

// SetLevel raises the minimum level that will be printed.
func SetLevel(l Level) { minLevel.Store(int32(l)) }

func Log(level Level, format string, args ...any) {
	if level != FatalLevel && int32(level) < minLevel.Load() {
		return
	}

	var prefix string
	switch level {
	case DebugLevel:
		prefix = "[DEBUG]"
	case InfoLevel:
		prefix = "[LOG]"
	case WarnLevel:
		prefix = "[WARNING]"
	case ErrorLevel, FatalLevel:
		prefix = "[ERROR]"
	default:
		prefix = "[LOG]"
	}

	msg := strings.TrimSuffix(fmt.Sprintf(format, args...), "\n")

	switch level {
	case FatalLevel:
		log.Fatalf("%s %s", prefix, msg)
	case ErrorLevel, WarnLevel:
		fmt.Fprintf(os.Stderr, "%s %s\n", prefix, msg)
	default:
		fmt.Printf("%s %s\n", prefix, msg)
	}
}
