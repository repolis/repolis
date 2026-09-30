import { describe, expect, it } from "vitest";

import { formatAge, formatCount, parseRepo } from "./format";

describe("parseRepo", () => {
  it("accepts every way a repository is usually pasted", () => {
    const want = { owner: "gin-gonic", repo: "gin" };
    expect(parseRepo("gin-gonic/gin")).toEqual(want);
    expect(parseRepo("github.com/gin-gonic/gin")).toEqual(want);
    expect(parseRepo("https://github.com/gin-gonic/gin")).toEqual(want);
    expect(parseRepo("https://www.github.com/gin-gonic/gin/")).toEqual(want);
    expect(parseRepo("https://github.com/gin-gonic/gin.git")).toEqual(want);
    expect(parseRepo("git@github.com:gin-gonic/gin.git")).toEqual(want);
  });

  it("ignores anything after owner/repo", () => {
    expect(parseRepo("https://github.com/a/b/tree/main/src")).toEqual({
      owner: "a",
      repo: "b",
    });
  });

  it("rejects input that is not a repository", () => {
    expect(parseRepo("")).toBeNull();
    expect(parseRepo("gin")).toBeNull();
    expect(parseRepo("owner/")).toBeNull();
    expect(parseRepo("own er/repo")).toBeNull();
  });
});

describe("formatting", () => {
  it("prints counts the way the HUD does", () => {
    expect(formatCount(81240)).toBe("81,240");
    expect(formatCount(3291.4)).toBe("3,291");
  });

  it("describes age in the largest sensible unit", () => {
    expect(formatAge(0)).toBe("unknown");
    expect(formatAge(12)).toBe("12d ago");
    expect(formatAge(90)).toBe("3mo ago");
    expect(formatAge(1095)).toBe("3.0y ago");
  });
});
