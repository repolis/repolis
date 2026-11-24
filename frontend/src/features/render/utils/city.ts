import type {
  BuildingInfo,
  CitySummary,
  DistrictInfo,
  HoverInfo,
  PathInfo,
  SelectPayload,
} from "../types";
import { scalesFor, type Scales } from "./appearance";

export interface RawCity {
  districts: RawDistrict[];
  dependencies: { source: string; target: string }[] | null;
  stats: RawStats;
}

type RawBuilding = Omit<
  BuildingInfo,
  "district_name" | "typology" | "calls" | "called_by"
>;

interface RawDistrict extends Omit<
  DistrictInfo,
  "building_count" | "total_lines_of_code" | "total_methods" | "top_buildings"
> {
  buildings: RawBuilding[] | null;
}

type RawStats = Omit<
  CitySummary,
  "languages" | "cycles" | "districts" | "index"
> & {
  languages: Record<string, number> | null;
  cycles:
    | {
        id: number;
        size: number;
        namespaces: number;
        members: string[];
        reliable: boolean;
      }[]
    | null;
};

export type EngineTarget =
  { type: "Building" | "District"; index: number } | null | undefined;
export interface PathQuery {
  from_id: string;
  to_id: string;
  hops: string[];
}

export interface City {
  summary: CitySummary;
  buildings: Map<string, BuildingInfo>;
  districts: Map<string, DistrictInfo>;
  /** City order, matching the engine's arrays. */
  list: BuildingInfo[];
  indexOf: Map<string, number>;
  adj: Map<string, Set<string>>;
  scales: Scales;
}

const byCountThenName = (a: [string, number], b: [string, number]) =>
  b[1] - a[1] || a[0].localeCompare(b[0]);

export function buildCity(raw: RawCity): City {
  const name = new Map<string, string>();
  for (const d of raw.districts)
    for (const b of d.buildings ?? []) name.set(b.id, b.name);

  const calls = new Map<string, string[]>();
  const calledBy = new Map<string, string[]>();
  const adj = new Map<string, Set<string>>();
  const push = (m: Map<string, string[]>, k: string, v: string) =>
    m.has(k) ? m.get(k)!.push(v) : m.set(k, [v]);
  const link = (a: string, b: string) =>
    adj.has(a) ? adj.get(a)!.add(b) : adj.set(a, new Set([b]));
  for (const e of raw.dependencies ?? []) {
    const t = name.get(e.target);
    const s = name.get(e.source);
    if (t !== undefined) push(calls, e.source, t);
    if (s !== undefined) push(calledBy, e.target, s);
    if (t !== undefined && s !== undefined && e.source !== e.target) {
      link(e.source, e.target);
      link(e.target, e.source);
    }
  }

  const buildings = new Map<string, BuildingInfo>();
  const districts = new Map<string, DistrictInfo>();
  const all: BuildingInfo[] = [];
  for (const d of raw.districts) {
    const list = d.buildings ?? [];
    for (const b of list) {
      const info = {
        ...b,
        fields: b.fields ?? [],
        methods: b.methods ?? [],
        district_name: d.name,
        typology: d.typology,
        calls: calls.get(b.id) ?? [],
        called_by: calledBy.get(b.id) ?? [],
      };
      buildings.set(b.id, info);
      all.push(info);
    }
    districts.set(d.id, {
      id: d.id,
      name: d.name,
      typology: d.typology,
      summary: d.summary,
      tags: d.tags ?? [],
      building_count: list.length,
      total_lines_of_code: list.reduce((n, b) => n + b.lines_of_code, 0),
      total_methods: list.reduce((n, b) => n + b.num_methods, 0),
      top_buildings: list
        .map((b): [string, number] => [
          b.name,
          b.num_methods * 3 + b.num_fields,
        ])
        .sort(byCountThenName)
        .slice(0, 8)
        .map(([n]) => n),
    });
  }

  const s = raw.stats;
  const summary: CitySummary = {
    ...s,
    skipped_dirs: s.skipped_dirs ?? [],
    languages: Object.entries(s.languages ?? {}).sort(byCountThenName),
    cycles: (s.cycles ?? [])
      .filter((c) => c.reliable && c.namespaces > 1)
      .map(({ id, size, namespaces, members }) => ({
        id,
        size,
        namespaces,
        members,
      })),
    districts: [...districts.values()],
    index: [...buildings.values()].map((b) => ({
      id: b.id,
      name: b.name,
      kind: b.kind,
      district: b.district_name,
    })),
  };

  return {
    summary,
    buildings,
    districts,
    list: all,
    indexOf: new Map(all.map((b, i) => [b.id, i])),
    adj,
    scales: scalesFor(all),
  };
}

export function selection(city: City, t: EngineTarget): SelectPayload {
  if (t?.type === "Building" && city.list[t.index])
    return { type: "Building", data: city.list[t.index] };
  if (t?.type === "District" && city.summary.districts[t.index])
    return { type: "District", data: city.summary.districts[t.index] };
  return { type: "None" };
}

export function hoverInfo(city: City, t: EngineTarget): HoverInfo | null {
  const s = selection(city, t);
  if (s.type === "Building")
    return {
      kind: "building",
      name: s.data.name,
      detail: `${s.data.kind}, ${s.data.num_fields} fields, ${s.data.num_methods} functions`,
    };
  if (s.type === "District")
    return {
      kind: "district",
      name: s.data.name,
      detail: `${s.data.building_count} entities, ${s.data.typology}`,
    };
  return null;
}

export function pathInfo(city: City, p: PathQuery): PathInfo {
  const nameOf = (id: string) => city.buildings.get(id)?.name ?? "";
  return {
    from_id: p.from_id,
    from_name: nameOf(p.from_id),
    to_id: p.to_id,
    to_name: nameOf(p.to_id),
    hops: p.hops.map(nameOf),
  };
}

export function resolveSymbol(city: City, query: string): string | null {
  if (city.buildings.has(query)) return query;
  const q = query.toLowerCase();
  let best: { id: string; score: number } | null = null;
  for (const b of city.buildings.values()) {
    const n = b.name.toLowerCase();
    const score = n === q ? 0 : n.startsWith(q) ? 1 : n.includes(q) ? 2 : -1;
    if (score >= 0 && (!best || score < best.score)) best = { id: b.id, score };
  }
  return best?.id ?? null;
}
