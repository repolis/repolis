import type {
  BuildingInfo,
  CitySummary,
  DistrictInfo,
  HoverInfo,
  PathInfo,
  SelectPayload,
} from "../types";
import { scalesFor, type Scales } from "./color-modes";

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
        loops?: string[][] | null;
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
  list: BuildingInfo[];
  indexOf: Map<string, number>;
  adj: Map<string, Set<string>>;
  scales: Scales;
}

const buildingSize = (b: RawBuilding) => b.num_methods * 3 + b.num_fields;

const byCountThenName = (a: [string, number], b: [string, number]) =>
  b[1] - a[1] || a[0].localeCompare(b[0]);

export function buildCity(raw: RawCity): City {
  const name = new Map<string, string>();
  for (const d of raw.districts)
    for (const b of d.buildings ?? []) name.set(b.id, b.name);

  const callees = new Map<string, string[]>();
  const callers = new Map<string, string[]>();
  const adj = new Map<string, Set<string>>();
  const append = (m: Map<string, string[]>, key: string, value: string) =>
    m.has(key) ? m.get(key)!.push(value) : m.set(key, [value]);
  const addTo = (m: Map<string, Set<string>>, key: string, value: string) =>
    m.has(key) ? m.get(key)!.add(value) : m.set(key, new Set([value]));

  for (const e of raw.dependencies ?? []) {
    if (!name.has(e.source) || !name.has(e.target)) continue;
    append(callees, e.source, e.target);
    append(callers, e.target, e.source);
    if (e.source === e.target) continue;
    addTo(adj, e.source, e.target);
    addTo(adj, e.target, e.source);
  }
  const namesOf = (ids: string[] | undefined) =>
    (ids ?? []).map((id) => name.get(id)!);

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
        calls: namesOf(callees.get(b.id)),
        called_by: namesOf(callers.get(b.id)),
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
        .map((b): [string, number] => [b.name, buildingSize(b)])
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
      .map(({ id, size, namespaces, members, loops }) => ({
        id,
        size,
        namespaces,
        members,
        loops: loops ?? [],
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

export function matchRank(name: string, lowerQuery: string): number | null {
  const lowerName = name.toLowerCase();
  if (lowerName === lowerQuery) return 0;
  if (lowerName.startsWith(lowerQuery)) return 1;
  if (lowerName.includes(lowerQuery)) return 2;
  return null;
}

export function resolveSymbol(city: City, query: string): string | null {
  if (city.buildings.has(query)) return query;
  const lowerQuery = query.toLowerCase();
  let best: { id: string; rank: number } | null = null;
  for (const b of city.buildings.values()) {
    const rank = matchRank(b.name, lowerQuery);
    if (rank !== null && (!best || rank < best.rank)) best = { id: b.id, rank };
  }
  return best?.id ?? null;
}
