import { describe, expect, it } from "vitest";

import { shortestPath } from "../utils/graph";
import { undirectedGraph } from "./fixtures";

describe("shortest path", () => {
  it("takes the short route, not merely a route", () => {
    const g = undirectedGraph([
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
        undirectedGraph([
          ["b", "a"],
          ["c", "b"],
        ]),
        "a",
        "c",
      ),
    ).toEqual(["a", "b", "c"]);
  });

  it("is empty for a disconnected or unknown pair", () => {
    const g = undirectedGraph([
      ["a", "b"],
      ["c", "d"],
    ]);
    expect(shortestPath(g, "a", "d")).toEqual([]);
    expect(shortestPath(g, "a", "ghost")).toEqual([]);
    expect(shortestPath(g, "ghost", "a")).toEqual([]);
  });

  it("reaches a building from itself in zero hops", () => {
    expect(shortestPath(undirectedGraph([["a", "b"]]), "a", "a")).toEqual([
      "a",
    ]);
  });
});
