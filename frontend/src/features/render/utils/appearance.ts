import type { BuildingInfo, CitySummary, FilterState } from "../types";
import type { City } from "./city";

type Rgb = [number, number, number];

const GREY: Rgb = [0.62, 0.62, 0.64];

export const TYPOLOGY_RGB: Record<string, Rgb> = {
  core: [0.46, 0.55, 0.72],
  data: [0.3, 0.62, 0.42],
  network: [0.28, 0.52, 0.78],
  security: [0.78, 0.35, 0.33],
  interface: [0.85, 0.66, 0.3],
  utility: [0.68, 0.62, 0.5],
  config: [0.52, 0.54, 0.62],
  test: [0.58, 0.4, 0.74],
  example: [0.38, 0.68, 0.72],
  unknown: GREY,
};

/** Distinct from every typology hue. */
export const LANGUAGE_RGB: Record<string, Rgb> = {
  C: [0.38, 0.55, 0.78],
  Rust: [0.8, 0.47, 0.28],
  Go: [0.35, 0.72, 0.76],
  unknown: GREY,
};

export type Smell = "god" | "brain" | "data";

export const SMELL_RGB: Record<Smell, Rgb> = {
  god: [0.82, 0.2, 0.22],
  brain: [0.9, 0.6, 0.2],
  data: [0.35, 0.55, 0.85],
};

const SMELL_LABELS: Record<Smell, string> = {
  god: "god class",
  brain: "very complex function",
  data: "data-only struct",
};

const NO_SMELL: Rgb = [0.3, 0.3, 0.32];

export const RAMP_STOPS: Rgb[] = [
  [0.86, 0.85, 0.78],
  [0.88, 0.72, 0.36],
  [0.85, 0.44, 0.22],
  [0.72, 0.16, 0.17],
];

export const css = ([r, g, b]: Rgb) =>
  `rgb(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)})`;

const cssTable = (t: Record<string, Rgb>) =>
  Object.fromEntries(Object.entries(t).map(([k, v]) => [k, css(v)]));

export const TYPOLOGY_COLORS = cssTable(TYPOLOGY_RGB);
export const LANGUAGE_COLORS = cssTable(LANGUAGE_RGB);
const SMELL_COLORS = cssTable(SMELL_RGB);
export const RAMP_CSS = `linear-gradient(to right, ${RAMP_STOPS.map(css).join(", ")})`;

const clamp01 = (v: number) => Math.min(Math.max(v, 0), 1);

export function ramp(t: number): Rgb {
  const position = clamp01(t) * (RAMP_STOPS.length - 1);
  const segment = Math.floor(position);
  const next = Math.min(segment + 1, RAMP_STOPS.length - 1);
  const within = position - segment;
  return RAMP_STOPS[segment].map(
    (c, channel) => c + (RAMP_STOPS[next][channel] - c) * within,
  ) as Rgb;
}

const RECENCY_HORIZON_DAYS = 540;

/** Buckets, not a smooth ramp: they compare better and keep the engine's materials bounded. */
const AGE_BUCKETS = [45, 180, 400, 900];

export function applyAge(rgb: Rgb, ageDays: number): Rgb {
  const bucketsPassed = AGE_BUCKETS.filter((edge) => ageDays >= edge).length;
  const greyMix = 0.55 * (bucketsPassed / AGE_BUCKETS.length);
  const grey = (rgb[0] + rgb[1] + rgb[2]) / 3;
  return rgb.map((c) => c * (1 - greyMix) + grey * greyMix) as Rgb;
}

const groundRgb = (typology: string): Rgb => {
  const [r, g, b] = TYPOLOGY_RGB[typology] ?? GREY;
  return [r * 0.16 + 0.05, g * 0.16 + 0.05, b * 0.16 + 0.06];
};

export interface Scales {
  complexity: number;
  fanIn: number;
  sumComplexity: number;
  fanOut: number;
  born: number;
}

function percentile(values: number[], percent: number): number | undefined {
  const sorted = [...values].sort((a, b) => a - b);
  const at = Math.floor((sorted.length * percent) / 100);
  return sorted[Math.min(at, sorted.length - 1)];
}

/** p95, not max: libgit2's worst function is 129 and nearly everything is under 15. */
export function scalesFor(buildings: BuildingInfo[]): Scales {
  const p95 = (pick: (b: BuildingInfo) => number) =>
    Math.max(percentile(buildings.map(pick), 95) ?? 20, 1);
  return {
    complexity: p95((b) => b.max_complexity),
    fanIn: p95((b) => b.fan_in),
    sumComplexity: p95((b) => b.sum_complexity),
    fanOut: p95((b) => b.fan_out),
    born: Math.max(1, ...buildings.map((b) => b.born_day)),
  };
}

/**
 * Rough stand-ins for Lanza & Marinescu's detection strategies, which need
 * metrics we do not collect. Floors keep small repos from flagging anything.
 */
const GOD_MIN_TOTAL_COMPLEXITY = 40;
const GOD_MIN_FAN_OUT = 5;
const GOD_MIN_METHODS = 8;
const BRAIN_MIN_COMPLEXITY = 15;
const DATA_MIN_FIELDS = 8;

export function smell(b: BuildingInfo, s: Scales): Smell | null {
  const isGod =
    b.sum_complexity >= Math.max(GOD_MIN_TOTAL_COMPLEXITY, s.sumComplexity) &&
    b.fan_out >= Math.max(GOD_MIN_FAN_OUT, s.fanOut) &&
    b.num_methods >= GOD_MIN_METHODS;
  if (isGod) return "god";
  if (b.max_complexity >= Math.max(BRAIN_MIN_COMPLEXITY, s.complexity))
    return "brain";
  const isDataOnly =
    b.kind === "type" && b.num_fields >= DATA_MIN_FIELDS && b.num_methods === 0;
  if (isDataOnly) return "data";
  return null;
}

export interface ColorMode {
  id: string;
  label: string;
  help?: string;
  ends?: [string, string];
  swatches?: (s: CitySummary) => [label: string, color: string][];
  color: (b: BuildingInfo, s: Scales) => Rgb;
}

export const COLOR_MODES: ColorMode[] = [
  {
    id: "typology",
    label: "Purpose",
    help: "Colour = district purpose.",
    swatches: (s) => {
      const used = new Set(s.districts.map((d) => d.typology));
      return Object.entries(TYPOLOGY_COLORS).filter(([t]) => used.has(t));
    },
    // Only here does age fade: in a numeric mode it would corrupt the ramp.
    color: (b) => applyAge(TYPOLOGY_RGB[b.typology] ?? GREY, b.age_days),
  },
  {
    id: "complexity",
    label: "Complexity",
    help: "Colour = worst cyclomatic complexity in the building. Pale is simple, red is branchy.",
    ends: ["simple", "branchy"],
    color: (b, s) => ramp(b.max_complexity / Math.max(s.complexity, 1)),
  },
  {
    id: "age",
    label: "Recency",
    help: "Colour = recency. Red was edited recently, pale has not changed in a long time.",
    ends: ["long untouched", "edited recently"],
    // Recent is hot, old is pale: the same direction as every other mode.
    color: (b) => ramp(1 - clamp01(b.age_days / RECENCY_HORIZON_DAYS)),
  },
  {
    id: "churn",
    label: "Churn",
    help: "Colour = commit churn percentile. Red is edited constantly.",
    ends: ["rarely edited", "edited constantly"],
    color: (b) => ramp(b.churn_rank),
  },
  {
    id: "fanin",
    label: "Depended on",
    help: "Colour = how many other buildings depend on this one. Red is depended on heavily.",
    ends: ["depended on by nothing", "depended on heavily"],
    color: (b, s) => ramp(b.fan_in / Math.max(s.fanIn, 1)),
  },
  {
    id: "instability",
    label: "Instability",
    help: "Colour = Martin's instability. Pale is depended upon, red depends on others.",
    ends: ["depended upon", "depends on others"],
    color: (b) => ramp(b.instability),
  },
  {
    id: "language",
    label: "Language",
    help: "Colour = source language.",
    swatches: (s) =>
      s.languages.map(([name, n]) => [
        `${name} ${n}`,
        LANGUAGE_COLORS[name] ?? LANGUAGE_COLORS.unknown,
      ]),
    color: (b) => LANGUAGE_RGB[b.language] ?? GREY,
  },
  {
    id: "smells",
    label: "Smells",
    help: "Colour = approximate design smell. Dark grey has none.",
    swatches: () =>
      Object.entries(SMELL_LABELS).map(([k, label]) => [
        label,
        SMELL_COLORS[k],
      ]),
    color: (b, s) => {
      const k = smell(b, s);
      return k ? SMELL_RGB[k] : NO_SMELL;
    },
  },
  {
    id: "born",
    label: "Birth",
    help: "Colour = when the file first appeared. Pale is original code, red was added recently, grey has no git history.",
    ends: ["original", "added recently"],
    color: (b, s) =>
      b.commit_churn === 0 ? GREY : ramp(b.born_day / Math.max(s.born, 1)),
  },
];

export const colorMode = (id: string) =>
  COLOR_MODES.find((m) => m.id === id) ?? COLOR_MODES[0];

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

function walkBack(
  prev: Map<string, string>,
  from: string,
  to: string,
): string[] {
  const path = [to];
  for (let at = to; at !== from;) {
    at = prev.get(at)!;
    path.push(at);
  }
  return path.reverse();
}

/** Hop count, ignoring direction: "three steps away" is actionable, a weighted cost is not. */
export function shortestPath(
  adj: Map<string, Set<string>>,
  from: string,
  to: string,
): string[] {
  const prev = new Map<string, string>([[from, from]]);
  const queue = [from];
  for (let head = 0; head < queue.length; head++) {
    const current = queue[head];
    if (current === to) return walkBack(prev, from, to);
    for (const next of adj.get(current) ?? []) {
      if (!prev.has(next)) {
        prev.set(next, current);
        queue.push(next);
      }
    }
  }
  return [];
}

function shortestLoopThrough(
  out: Map<string, Set<string>>,
  members: Set<string>,
  start: string,
): string[] | null {
  const prev = new Map<string, string>([[start, start]]);
  const queue = [start];
  for (let head = 0; head < queue.length; head++) {
    const current = queue[head];
    for (const next of out.get(current) ?? []) {
      if (next === start) return walkBack(prev, start, current);
      if (members.has(next) && !prev.has(next)) {
        prev.set(next, current);
        queue.push(next);
      }
    }
  }
  return null;
}

function rotateToSmallestId(loop: string[]): string[] {
  const smallest = loop.indexOf([...loop].sort()[0]);
  return [...loop.slice(smallest), ...loop.slice(0, smallest)];
}

export function loopsOf(
  out: Map<string, Set<string>>,
  members: string[],
): string[][] {
  const inside = new Set(members);
  const unique = new Map<string, string[]>();
  for (const start of members) {
    const loop = shortestLoopThrough(out, inside, start);
    if (!loop) continue;
    const rotated = rotateToSmallestId(loop);
    unique.set(rotated.join("\n"), rotated);
  }
  return [...unique.values()].sort((a, b) => a.length - b.length);
}

export const closedLoop = (loop: string[]) => [...loop, loop[0]];

const NORMAL = 0;
const DIM = 1;
const BRIGHT = 2;
const FADED = 3;

export function isVisible(
  b: BuildingInfo,
  filter: FilterState,
  timeline: number | null,
): boolean {
  return (timeline === null || b.born_day <= timeline) && matches(filter, b);
}

export interface View {
  mode: string;
  filter: FilterState;
  timeline: number | null;
  selectedId: string | null;
  path: string[];
}

function cyclePairs(city: City, states: Uint8Array): string[][] {
  const shown = (id: string) => {
    const index = city.indexOf.get(id);
    return index !== undefined && states[index] !== FADED;
  };
  const pairs: string[][] = [];
  for (const b of city.list) {
    if (b.cycle_id === 0 || !shown(b.id)) continue;
    for (const neighbour of city.adj.get(b.id) ?? []) {
      const countedOnce = b.id < neighbour;
      const sameCycle = city.buildings.get(neighbour)?.cycle_id === b.cycle_id;
      if (countedOnce && sameCycle && shown(neighbour))
        pairs.push([b.id, neighbour]);
    }
  }
  return pairs;
}

function stateOf(
  b: BuildingInfo,
  v: View,
  onPath: Set<string>,
  related: Set<string>,
): number {
  // Faded rather than gone: the timeline is for watching it get built.
  if (!isVisible(b, v.filter, v.timeline)) return FADED;
  if (onPath.size > 0) return onPath.has(b.id) ? BRIGHT : DIM;
  if (!v.selectedId) return NORMAL;
  if (b.id === v.selectedId) return BRIGHT;
  return related.has(b.id) ? NORMAL : DIM;
}

// The path replaces the neighbour highlight; both at once hides the answer.
function highlightedPairs(
  city: City,
  v: View,
  related: Set<string>,
  states: Uint8Array,
): string[][] {
  if (v.path.length > 0) return v.path.slice(1).map((id, i) => [v.path[i], id]);
  if (v.selectedId) return [...related].map((id) => [v.selectedId!, id]);
  if (v.filter.only_cycles) return cyclePairs(city, states);
  return [];
}

export function computeAppearance(city: City, v: View) {
  const { color } = colorMode(v.mode);
  const colors = new Float32Array(city.list.length * 3);
  const states = new Uint8Array(city.list.length);
  const onPath = new Set(v.path);
  const related: Set<string> =
    (v.selectedId && city.adj.get(v.selectedId)) || new Set();

  city.list.forEach((b, i) => {
    colors.set(color(b, city.scales), i * 3);
    states[i] = stateOf(b, v, onPath, related);
  });

  const pairs = highlightedPairs(city, v, related, states);
  const links = new Uint32Array(
    pairs.flat().map((id) => city.indexOf.get(id) ?? 0),
  );

  const grounds = new Float32Array(
    city.summary.districts.flatMap((d) => groundRgb(d.typology)),
  );
  return { colors, states, grounds, links };
}
