import type { BuildingInfo } from "../types";

export const widget = {
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

export const SCALES = {
  complexity: 1,
  fanIn: 1,
  born: 1,
};

export const undirectedGraph = (pairs: [string, string][]) => {
  const adj = new Map<string, Set<string>>();
  for (const [a, b] of pairs) {
    adj.set(a, (adj.get(a) ?? new Set()).add(b));
    adj.set(b, (adj.get(b) ?? new Set()).add(a));
  }
  return adj;
};
