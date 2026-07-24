package git

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
)

func CloneRepo(repoURL string, sessionID string) (string, error) {
	clonePath, err := os.MkdirTemp("", "repolis-session-"+sessionID+"-*")
	if err != nil {
		return "", err
	}

	cmd := exec.Command("git", "clone", "--depth", "1", repoURL, clonePath)
	if err := cmd.Run(); err != nil {
		os.RemoveAll(clonePath)
		return "", err
	}

	return clonePath, nil
}

func GetRemoteCommitHash(repoURL string) (string, error) {
	cmd := exec.Command("git", "ls-remote", repoURL, "HEAD")
	out, err := cmd.Output()
	if err != nil {
		return "", err
	}
	
	parts := strings.Fields(string(out))
	if len(parts) > 0 {
		return parts[0], nil
	}
	return "", fmt.Errorf("could not parse ls-remote output")
}
