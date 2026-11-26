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

export function ramp(t: number): Rgb {
  const scaled = Math.min(Math.max(t, 0), 1) * (RAMP_STOPS.length - 1);
  const i = Math.floor(scaled);
  const j = Math.min(i + 1, RAMP_STOPS.length - 1);
  const f = scaled - i;
  return RAMP_STOPS[i].map((c, k) => c + (RAMP_STOPS[j][k] - c) * f) as Rgb;
}

/** Buckets, not a smooth ramp: they compare better and keep the engine's materials bounded. */
const AGE_BUCKETS = [45, 180, 400, 900];

export function applyAge(rgb: Rgb, ageDays: number): Rgb {
  const t =
    AGE_BUCKETS.filter((edge) => ageDays >= edge).length / AGE_BUCKETS.length;
  const grey = (rgb[0] + rgb[1] + rgb[2]) / 3;
  const mix = 0.55 * t;
  return rgb.map((c) => c * (1 - mix) + grey * mix) as Rgb;
}

const groundRgb = (typology: string): Rgb => {
  const [r, g, b] = TYPOLOGY_RGB[typology] ?? GREY;
  return [r * 0.16 + 0.05, g * 0.16 + 0.05, b * 0.16 + 0.06];
};

/** p95, not max: libgit2's worst function is 129 and nearly everything is under 15. */
export interface Scales {
  complexity: number;
  fanIn: number;
  sumComplexity: number;
  fanOut: number;
  born: number;
}

export function scalesFor(buildings: BuildingInfo[]): Scales {
  const p95 = (pick: (b: BuildingInfo) => number) => {
    const v = buildings.map(pick).sort((a, b) => a - b);
    if (v.length === 0) return 20;
    return Math.max(
      v[Math.min(Math.floor((v.length * 95) / 100), v.length - 1)],
      1,
    );
  };
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
export function smell(b: BuildingInfo, s: Scales): Smell | null {
  if (
    b.sum_complexity >= Math.max(40, s.sumComplexity) &&
    b.fan_out >= Math.max(5, s.fanOut) &&
    b.num_methods >= 8
  )
    return "god";
  if (b.max_complexity >= Math.max(15, s.complexity)) return "brain";
  if (b.kind === "type" && b.num_fields >= 8 && b.num_methods === 0)
    return "data";
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
    color: (b) => ramp(1 - Math.min(Math.max(b.age_days / 540, 0), 1)),
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

/** Hop count, ignoring direction: "three steps away" is actionable, a weighted cost is not. */
export function shortestPath(
  adj: Map<string, Set<string>>,
  from: string,
  to: string,
): string[] {
  const prev = new Map<string, string>([[from, from]]);
  const queue = [from];
  for (let head = 0; head < queue.length; head++) {
    const cur = queue[head];
    if (cur === to) {
      const path = [cur];
      for (let at = cur; prev.get(at) !== at;) path.push((at = prev.get(at)!));
      return path.reverse();
    }
    for (const next of adj.get(cur) ?? []) {
      if (!prev.has(next)) {
        prev.set(next, cur);
        queue.push(next);
      }
    }
  }
  return [];
}

export function loopsOf(
  out: Map<string, Set<string>>,
  members: string[],
): string[][] {
  const inside = new Set(members);
  const loops = new Map<string, string[]>();
  for (const start of members) {
    const prev = new Map<string, string>();
    const queue = [start];
    let last: string | undefined;
    for (let head = 0; head < queue.length && !last; head++) {
      for (const next of out.get(queue[head]) ?? []) {
        if (next === start) {
          last = queue[head];
          break;
        }
        if (inside.has(next) && !prev.has(next)) {
          prev.set(next, queue[head]);
          queue.push(next);
        }
      }
    }
    if (!last) continue;
    const loop = [last];
    while (loop[0] !== start) loop.unshift(prev.get(loop[0]) ?? start);
    // Rotated to its smallest id so the same loop found from two members dedupes.
    const at = loop.indexOf([...loop].sort()[0]);
    const key = [...loop.slice(at), ...loop.slice(0, at)];
    loops.set(key.join("\n"), key);
  }
  return [...loops.values()].sort((a, b) => a.length - b.length);
}

const NORMAL = 0;
const DIM = 1;
const BRIGHT = 2;
const FADED = 3;

export interface View {
  mode: string;
  filter: FilterState;
  timeline: number | null;
  selectedId: string | null;
  path: string[];
}

function cyclePairs(city: City, states: Uint8Array): string[][] {
  const visible = (id: string) => states[city.indexOf.get(id) ?? 0] !== FADED;
  return city.list.flatMap((b) =>
    b.cycle_id === 0 || !visible(b.id)
      ? []
      : [...(city.adj.get(b.id) ?? [])]
          .filter(
            (n) =>
              n > b.id &&
              visible(n) &&
              city.buildings.get(n)?.cycle_id === b.cycle_id,
          )
          .map((n) => [b.id, n]),
  );
}

export function computeAppearance(city: City, v: View) {
  const { color } = colorMode(v.mode);
  const colors = new Float32Array(city.list.length * 3);
  const states = new Uint8Array(city.list.length);
  const onPath = new Set(v.path);
  const related = (v.selectedId && city.adj.get(v.selectedId)) || new Set();

  city.list.forEach((b, i) => {
    colors.set(color(b, city.scales), i * 3);
    states[i] =
      // Faded rather than gone: the timeline is for watching it get built.
      (v.timeline !== null && b.born_day > v.timeline) || !matches(v.filter, b)
        ? FADED
        : onPath.size > 0
          ? onPath.has(b.id)
            ? BRIGHT
            : DIM
          : v.selectedId
            ? b.id === v.selectedId
              ? BRIGHT
              : related.has(b.id)
                ? NORMAL
                : DIM
            : NORMAL;
  });

  // The path replaces the neighbour highlight; both at once hides the answer.
  const pairs =
    onPath.size > 0
      ? v.path.slice(1).map((id, i) => [v.path[i], id])
      : v.selectedId
        ? [...related].map((id) => [v.selectedId!, id])
        : v.filter.only_cycles
          ? cyclePairs(city, states)
          : [];
  const links = new Uint32Array(
    pairs.flat().map((id) => city.indexOf.get(id) ?? 0),
  );

  const grounds = new Float32Array(
    city.summary.districts.flatMap((d) => groundRgb(d.typology)),
  );
  return { colors, states, grounds, links };
}
