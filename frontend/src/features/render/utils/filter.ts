import type { BuildingInfo, FilterState } from "../types";

export const EMPTY_FILTER: FilterState = {
  text: "",
  language: "",
  district: "",
  kind: "",
  min_methods: 0,
  min_complexity: 0,
  min_churn_pct: 0,
  only_no_callers: false,
  only_hubs: false,
  only_cycles: false,
};

export function matches(f: FilterState, b: BuildingInfo): boolean {
  return (
    (!f.text || b.name.toLowerCase().includes(f.text.toLowerCase())) &&
    (!f.language || b.language === f.language) &&
    (!f.district || b.district_name === f.district) &&
    (!f.kind || b.kind === f.kind) &&
    b.num_methods >= f.min_methods &&
    b.max_complexity >= f.min_complexity &&
    b.churn_rank * 100 >= f.min_churn_pct &&
    // A hint, not a verdict: a library's public surface has no internal callers.
    (!f.only_no_callers || b.fan_in === 0) &&
    (!f.only_hubs || b.hub) &&
    (!f.only_cycles || b.cycle_id !== 0)
  );
}

export function isVisible(
  b: BuildingInfo,
  filter: FilterState,
  timeline: number | null,
): boolean {
  return (timeline === null || b.born_day <= timeline) && matches(filter, b);
}
