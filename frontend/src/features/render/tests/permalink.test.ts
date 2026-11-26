import { describe, expect, it } from "vitest";

import { pinnedPath, splitCommit } from "../utils/permalink";

const SHA = "0123456789abcdef0123456789abcdef01234567";

describe("permalink", () => {
  it("reads a plain repository as the latest commit", () => {
    expect(splitCommit("sqlite")).toEqual({ repo: "sqlite", commit: null });
  });

  it("splits the commit off a pinned repository", () => {
    expect(splitCommit(`sqlite@${SHA}`)).toEqual({
      repo: "sqlite",
      commit: SHA,
    });
  });

  it("round-trips through the pinned path", () => {
    const path = pinnedPath("github.com", "sqlite", "sqlite", SHA);
    expect(path).toBe(`/city/github.com/sqlite/sqlite@${SHA}`);
    expect(splitCommit(path.split("/").pop()!)).toEqual({
      repo: "sqlite",
      commit: SHA,
    });
  });
});
