/**
 * What every building looks like: colour mode, filter, timeline, selection and
 * path. The engine only applies the result (`set_appearance`).
 *
 * Colour is the strongest channel, and height, footprint and saturation are
 * already taken - another simultaneous channel would be unreadable. Switching
 * colour gives each metric a full-strength palette in turn instead.
 */
import type { City } from "./city";
import type { BuildingInfo } from "./types";
import type { FilterState } from "./view-controls";

type Rgb = [number, number, number];

const GREY: Rgb = [0.62, 0.62, 0.64];

/** Hue carries the district's purpose. */
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

/** Categorical colour for a language, distinct from every typology hue. */
export const LANGUAGE_RGB: Record<string, Rgb> = {
  C: [0.38, 0.55, 0.78],
  Rust: [0.8, 0.47, 0.28],
  Go: [0.35, 0.72, 0.76],
  unknown: GREY,
};

/** Pale sand -> amber -> red, shared by every numeric mode so "more" always
 * looks the same. */
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

/** CSS versions for the legend and inspector, so they cannot drift. */
export const TYPOLOGY_COLORS = cssTable(TYPOLOGY_RGB);
export const LANGUAGE_COLORS = cssTable(LANGUAGE_RGB);
export const RAMP_CSS = `linear-gradient(to right, ${RAMP_STOPS.map(css).join(", ")})`;

export function ramp(t: number): Rgb {
  const scaled = Math.min(Math.max(t, 0), 1) * (RAMP_STOPS.length - 1);
  const i = Math.floor(scaled);
  const j = Math.min(i + 1, RAMP_STOPS.length - 1);
  const f = scaled - i;
  return RAMP_STOPS[i].map((c, k) => c + (RAMP_STOPS[j][k] - c) * f) as Rgb;
}

/** Age buckets, in days. Five distinguishable shades compare better across a
 * city than a smooth ramp, and keep the engine's materials bounded. */
const AGE_BUCKETS = [45, 180, 400, 900];

/** Untouched code fades toward concrete grey; recent code keeps its hue. */
export function applyAge(rgb: Rgb, ageDays: number): Rgb {
  const t =
    AGE_BUCKETS.filter((edge) => ageDays >= edge).length / AGE_BUCKETS.length;
  const grey = (rgb[0] + rgb[1] + rgb[2]) / 3;
  const mix = 0.55 * t;
  return rgb.map((c) => c * (1 - mix) + grey * mix) as Rgb;
}

const groundRgb = (typology: string): Rgb => {
  const [r, g, b] = TYPOLOGY_RGB[typology] ?? GREY;
  return [r * 0.3 + 0.05, g * 0.3 + 0.05, b * 0.3 + 0.06];
};

/** Scale for the numeric modes, set once per city. Percentile, not maximum:
 * libgit2's worst function is 129 and nearly everything is under 15. */
export interface Scales {
  complexity: number;
  fanIn: number;
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
  };
}

export function colorFor(mode: string, b: BuildingInfo, s: Scales): Rgb {
  switch (mode) {
    case "complexity":
      return ramp(b.max_complexity / Math.max(s.complexity, 1));
    // Recent is hot, old is pale: the same direction as every other mode.
    case "age":
      return ramp(1 - Math.min(Math.max(b.age_days / 540, 0), 1));
    case "churn":
      return ramp(b.churn_rank);
    case "fanin":
      return ramp(b.fan_in / Math.max(s.fanIn, 1));
    case "instability":
      return ramp(b.instability);
    case "language":
      return LANGUAGE_RGB[b.language] ?? GREY;
    // Only here does age fade the colour: in a numeric mode the ramp carries
    // the meaning, and fading it would corrupt it.
    default:
      return applyAge(TYPOLOGY_RGB[b.typology] ?? GREY, b.age_days);
  }
}

/** Every predicate must hold. An empty filter matches everything. */
export function matches(f: FilterState, b: BuildingInfo): boolean {
  return (
    (!f.text || b.name.toLowerCase().includes(f.text.toLowerCase())) &&
    (!f.language || b.language === f.language) &&
    (!f.district || b.district_name === f.district) &&
    (!f.kind || b.kind === f.kind) &&
    b.num_methods >= f.min_methods &&
    b.max_complexity >= f.min_complexity &&
    b.churn_rank * 100 >= f.min_churn_pct &&
    // A question, not a verdict: a library's whole public surface has no
    // internal callers.
    (!f.only_no_callers || b.fan_in === 0) &&
    (!f.only_hubs || b.hub) &&
    (!f.only_cycles || b.cycle_id !== 0)
  );
}

/** Shortest dependency path by hop count, ignoring direction: "three steps
 * away" is actionable where a weighted cost is not. Empty when unreachable. */
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

// Building states, as the engine reads them.
const NORMAL = 0;
const DIM = 1;
const BRIGHT = 2;
const FADED = 3;

export interface View {
  mode: string;
  filter: FilterState;
  /** Day of history to show the city as of, null for the present. */
  timeline: number | null;
  selectedId: string | null;
  /** Building ids on the highlighted path, in order. */
  path: string[];
}

/** The arrays `set_appearance` takes, in the city order both sides share. */
export function computeAppearance(city: City, v: View) {
  const colors = new Float32Array(city.list.length * 3);
  const states = new Uint8Array(city.list.length);
  const onPath = new Set(v.path);
  const related = (v.selectedId && city.adj.get(v.selectedId)) || new Set();

  city.list.forEach((b, i) => {
    colors.set(colorFor(v.mode, b, city.scales), i * 3);
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
  const links =
    onPath.size > 0
      ? v.path.slice(1).flatMap((id, i) => [v.path[i], id])
      : v.selectedId
        ? [...related].flatMap((id) => [v.selectedId!, id])
        : [];

  const grounds = new Float32Array(
    city.summary.districts.flatMap((d) => groundRgb(d.typology)),
  );
  return { colors, states, grounds, links };
}
