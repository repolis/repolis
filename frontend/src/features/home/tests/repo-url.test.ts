import { describe, expect, it } from "vitest";

import { parseRepoUrl } from "../utils/repo-url";

describe("parseRepoUrl", () => {
  it.each([
    ["https://github.com/tsoding/nothing", "github.com", "nothing"],
    ["https://www.gitlab.com/inkscape/inkscape", "gitlab.com", "inkscape"],
    ["https://codeberg.org/forgejo/forgejo/", "codeberg.org", "forgejo"],
    ["https://github.com/BurntSushi/ripgrep.git", "github.com", "ripgrep"],
    ["https://github.com/gin-gonic/gin?tab=readme#x", "github.com", "gin"],
  ])("accepts %s", (url, host, repo) => {
    expect(parseRepoUrl(url)).toMatchObject({ host, repo });
  });

  it.each([
    "http://github.com/a/b",
    "https://example.com/a/b",
    "https://gitlab.com/group/sub/repo",
    "https://github.com/a",
    "https://github.com/../b",
    "not a url",
  ])("rejects %s", (url) => {
    expect(parseRepoUrl(url)).toHaveProperty("error");
  });
});
