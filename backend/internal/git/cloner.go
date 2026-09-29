package git

import (
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"regexp"
	"strings"
)

// allowedHosts is the allow-list of forges we will talk to.
var allowedHosts = map[string]bool{
	"github.com":     true,
	"www.github.com": true,
	"gitlab.com":     true,
	"www.gitlab.com": true,
	"codeberg.org":   true,
	"bitbucket.org":  true,
}

var repoPathRe = regexp.MustCompile(`^/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+?(\.git)?$`)

// NormalizeRepoURL validates a user-supplied URL into canonical https form.
// A security boundary, not a convenience check: git reads some strings as
// shell-executing transports (`ext::sh -c ...`) and a leading "-" as a flag,
// so unvalidated input to `git clone` is remote code execution.
func NormalizeRepoURL(raw string) (string, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" || len(raw) > 512 {
		return "", fmt.Errorf("invalid repository URL")
	}
	if strings.HasPrefix(raw, "-") {
		return "", fmt.Errorf("invalid repository URL")
	}

	u, err := url.Parse(raw)
	if err != nil {
		return "", fmt.Errorf("invalid repository URL")
	}
	if u.Scheme != "https" {
		return "", fmt.Errorf("only https:// repository URLs are supported")
	}
	if u.User != nil {
		return "", fmt.Errorf("credentials in the URL are not supported")
	}
	host := strings.ToLower(u.Hostname())
	if !allowedHosts[host] {
		return "", fmt.Errorf("host %q is not allowed", host)
	}

	path := strings.TrimSuffix(u.Path, "/")
	if !repoPathRe.MatchString(path) {
		return "", fmt.Errorf("URL must be of the form https://%s/<owner>/<repo>", host)
	}
	if strings.Contains(path, "..") {
		return "", fmt.Errorf("invalid repository path")
	}

	return "https://" + host + strings.TrimSuffix(path, ".git"), nil
}

// gitEnv restricts git to https, closing off ext::/file:: transports even if
// a URL somehow slips past NormalizeRepoURL.
func gitEnv() []string {
	return append(os.Environ(),
		"GIT_ALLOW_PROTOCOL=https",
		"GIT_TERMINAL_PROMPT=0",
		"GIT_CONFIG_NOSYSTEM=1",
	)
}

// CloneRepo makes a blobless partial clone: the full commit and tree history
// ExtractHistory needs, blobs fetched lazily. Unlike --depth it keeps churn
// accurate.
func CloneRepo(repoURL string, sessionID string) (string, error) {
	clonePath, err := os.MkdirTemp("", "repolis-session-"+sessionID+"-*")
	if err != nil {
		return "", err
	}

	cmd := exec.Command("git", "clone",
		"--filter=blob:none",
		"--single-branch",
		"--quiet",
		"--", repoURL, clonePath,
	)
	cmd.Env = gitEnv()

	if out, err := cmd.CombinedOutput(); err != nil {
		os.RemoveAll(clonePath)
		return "", fmt.Errorf("git clone failed: %v: %s", err, strings.TrimSpace(string(out)))
	}
	return clonePath, nil
}

func GetRemoteCommitHash(repoURL string) (string, error) {
	cmd := exec.Command("git", "ls-remote", "--", repoURL, "HEAD")
	cmd.Env = gitEnv()

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
