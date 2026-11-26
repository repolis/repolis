import { describe, expect, it } from "vitest";

import { computeAppearance } from "../utils/appearance";
import type { City } from "../utils/city";
import { EMPTY_FILTER } from "../utils/filter";
import { SCALES, undirectedGraph, widget } from "./fixtures";

describe("cycle filter", () => {
  it("highlights only edges inside a cycle", () => {
    const list = [
      { ...widget, id: "a", cycle_id: 1 },
      { ...widget, id: "b", cycle_id: 1 },
      { ...widget, id: "c", cycle_id: 0 },
    ];
    const city = {
      list,
      indexOf: new Map(list.map((b, i) => [b.id, i])),
      buildings: new Map(list.map((b) => [b.id, b])),
      adj: undirectedGraph([
        ["a", "b"],
        ["a", "c"],
      ]),
      scales: SCALES,
      summary: { districts: [] },
    } as unknown as City;
    const { links } = computeAppearance(city, {
      mode: "typology",
      filter: { ...EMPTY_FILTER, only_cycles: true },
      timeline: null,
      selectedId: null,
      path: [],
    });
    expect([...links]).toEqual([0, 1]);
  });
});
