export interface BuildingInfo {
  id: string;
  name: string;
  kind: "type" | "module" | string;
  source_file: string;
  dir: string;
  language: string;
  district_name: string;
  typology: string;
  num_fields: number;
  num_methods: number;
  lines_of_code: number;
  /** Worst cyclomatic complexity among this building's functions. */
  max_complexity: number;
  sum_complexity: number;
  /** How many other buildings depend on this one, and it on them. */
  fan_in: number;
  fan_out: number;
  /** Martin's instability: 0 depended upon, 1 depends on others. */
  instability: number;
  /** In the top few percent by fan-in for this repository. */
  hub: boolean;
  commit_churn: number;
  churn_rank: number;
  last_modified: string;
  age_days: number;
  born_day: number;
  primary_author: string;
  summary: string;
  /** How this building's functions were attributed: rule | llm | none. */
  assoc_source: string;
  /** Dependency cycle this building belongs to, or 0. */
  cycle_id: number;
  fields: string[];
  methods: string[];
  calls: string[];
  called_by: string[];
}

export interface DistrictInfo {
  id: string;
  name: string;
  typology: string;
  summary: string;
  tags: string[];
  building_count: number;
  total_lines_of_code: number;
  total_methods: number;
  top_buildings: string[];
}

export interface IndexEntry {
  id: string;
  name: string;
  kind: string;
  district: string;
}

export interface CitySummary {
  total_buildings: number;
  total_types: number;
  total_modules: number;
  total_methods: number;
  total_files: number;
  total_loc: number;
  languages: [string, number][];
  cycles: CycleInfo[];
  history_days: number;
  first_commit: string;
  last_commit: string;
  methods_by_rule: number;
  methods_by_llm: number;
  orphans: number;
  llm_calls: number;
  refined: boolean;
  skipped_dirs: string[];
  districts: DistrictInfo[];
  index: IndexEntry[];
}

export interface CycleInfo {
  id: number;
  size: number;
  namespaces: number;
  members: string[];
}

export interface PathInfo {
  from_id: string;
  from_name: string;
  to_id: string;
  to_name: string;
  /** Building names in order. Empty when no path exists. */
  hops: string[];
}

export interface HoverInfo {
  kind: string;
  name: string;
  detail: string;
}

export type SelectPayload =
  | { type: "Building"; data: BuildingInfo }
  | { type: "District"; data: DistrictInfo }
  | { type: "None" };

/** Matches the Rust `typology_rgb` table so the legend cannot drift from the scene. */
export const TYPOLOGY_COLORS: Record<string, string> = {
  core: "rgb(117,140,184)",
  data: "rgb(77,158,107)",
  network: "rgb(71,133,199)",
  security: "rgb(199,89,84)",
  interface: "rgb(217,168,77)",
  utility: "rgb(173,158,128)",
  config: "rgb(133,138,158)",
  test: "rgb(148,102,189)",
  example: "rgb(97,173,184)",
  unknown: "rgb(158,158,163)",
};
