import { describe, expect, it } from "vitest";

import type { BuildingInfo } from "../types";
import {
  applyAge,
  colorMode,
  EMPTY_FILTER,
  matches,
  ramp,
  shortestPath,
} from "../utils/appearance";

const widget = {
  name: "Widget",
  kind: "type",
  language: "C",
  district_name: "Core",
  num_methods: 4,
  max_complexity: 9,
  fan_in: 3,
  churn_rank: 0.5,
  hub: false,
  cycle_id: 0,
} as BuildingInfo;

const graph = (pairs: [string, string][]) => {
  const adj = new Map<string, Set<string>>();
  for (const [a, b] of pairs) {
    adj.set(a, (adj.get(a) ?? new Set()).add(b));
    adj.set(b, (adj.get(b) ?? new Set()).add(a));
  }
  return adj;
};

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

describe("colour", () => {
  it("ramps darker toward hot and stays in range", () => {
    const [a, b] = [ramp(0), ramp(1)];
    expect(b[0] < a[0] || b[2] < a[2]).toBe(true);
    for (const t of [-1, 0, 0.5, 1, 2])
      for (const c of ramp(t)) expect(c >= 0 && c <= 1).toBe(true);
  });

  it("leaves recent code alone and greys old code", () => {
    expect(applyAge([0.8, 0.2, 0.2], 10)).toEqual([0.8, 0.2, 0.2]);
    const old = applyAge([0.8, 0.2, 0.2], 2000);
    expect(old[0] - old[1]).toBeLessThan(0.6);
  });
});

describe("birth mode", () => {
  const born = colorMode("born");
  const scales = { complexity: 1, fanIn: 1, born: 300 };

  it("puts the newest file at the hot end", () => {
    const b = { ...widget, commit_churn: 2, born_day: 300 };
    expect(born.color(b, scales)).toEqual(ramp(1));
  });

  it("does not paint files without history as the oldest", () => {
    const b = { ...widget, commit_churn: 0, born_day: 0 };
    expect(born.color(b, scales)).not.toEqual(ramp(0));
  });
});

describe("shortest path", () => {
  it("takes the short route, not merely a route", () => {
    const g = graph([
      ["a", "b"],
      ["b", "c"],
      ["c", "d"],
      ["a", "x"],
      ["x", "d"],
    ]);
    expect(shortestPath(g, "a", "d")).toEqual(["a", "x", "d"]);
  });

  it("follows edges in both directions", () => {
    expect(
      shortestPath(
        graph([
          ["b", "a"],
          ["c", "b"],
        ]),
        "a",
        "c",
      ),
    ).toEqual(["a", "b", "c"]);
  });

  it("is empty for a disconnected or unknown pair", () => {
    const g = graph([
      ["a", "b"],
      ["c", "d"],
    ]);
    expect(shortestPath(g, "a", "d")).toEqual([]);
    expect(shortestPath(g, "a", "ghost")).toEqual([]);
    expect(shortestPath(g, "ghost", "a")).toEqual([]);
  });

  it("reaches a building from itself in zero hops", () => {
    expect(shortestPath(graph([["a", "b"]]), "a", "a")).toEqual(["a"]);
  });
});
