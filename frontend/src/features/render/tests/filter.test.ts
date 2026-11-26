import { describe, expect, it } from "vitest";

import { EMPTY_FILTER, matches } from "../utils/filter";
import { widget } from "./fixtures";

describe("filter", () => {
  it("matches everything when empty", () => {
    expect(matches(EMPTY_FILTER, widget)).toBe(true);
  });

  it("composes predicates with and", () => {
    const f = { ...EMPTY_FILTER, language: "C", min_methods: 4 };
    expect(matches(f, widget)).toBe(true);
    expect(matches({ ...f, min_methods: 5 }, widget)).toBe(false);
    expect(matches({ ...f, language: "Rust" }, widget)).toBe(false);
  });

  it("matches text as a case-insensitive substring", () => {
    expect(matches({ ...EMPTY_FILTER, text: "wid" }, widget)).toBe(true);
    expect(matches({ ...EMPTY_FILTER, text: "nope" }, widget)).toBe(false);
  });

  it("selects only unreferenced buildings for no-callers", () => {
    const f = { ...EMPTY_FILTER, only_no_callers: true };
    expect(matches(f, widget)).toBe(false);
    expect(matches(f, { ...widget, fan_in: 0 })).toBe(true);
  });
});
