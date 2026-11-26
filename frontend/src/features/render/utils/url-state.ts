import type { FilterState } from "../types";
import { EMPTY_FILTER } from "./filter";

// In the fragment so it never reaches the server.
export interface ViewURL {
  mode: string;
  filter: FilterState;
  selected: string | null;
  timeline: number | null;
  camera: string | null;
}

export const EMPTY_VIEW: ViewURL = {
  mode: "typology",
  filter: EMPTY_FILTER,
  selected: null,
  timeline: null,
  camera: null,
};

// A new filter field needs one entry here.
const FILTER_CODES: Record<keyof FilterState, string> = {
  text: "q",
  language: "lang",
  district: "dist",
  kind: "kind",
  min_methods: "fn",
  min_complexity: "cx",
  min_churn_pct: "churn",
  only_no_callers: "nocallers",
  only_hubs: "hubs",
  only_cycles: "cycles",
};
const FILTER_KEYS = Object.keys(FILTER_CODES) as (keyof FilterState)[];

// Only non-default fields: a flag as its bare name, anything else as `name:value`.
function encodeFilter(f: FilterState): string {
  return FILTER_KEYS.filter((k) => f[k] !== EMPTY_FILTER[k])
    .map((k) =>
      typeof f[k] === "boolean"
        ? FILTER_CODES[k]
        : `${FILTER_CODES[k]}:${f[k]}`,
    )
    .join(";");
}

function decodeFilter(s: string): FilterState {
  const f: Record<string, unknown> = { ...EMPTY_FILTER };
  for (const part of s.split(";")) {
    const [code, ...rest] = part.split(":");
    const key = FILTER_KEYS.find((k) => FILTER_CODES[k] === code);
    if (!key) continue;
    const value = rest.join(":");
    switch (typeof EMPTY_FILTER[key]) {
      case "boolean":
        f[key] = true;
        break;
      case "number": {
        // A truncated or hand-edited link degrades to "no limit".
        const n = Number(value);
        f[key] = Number.isFinite(n) && n > 0 ? n : 0;
        break;
      }
      default:
        f[key] = value;
    }
  }
  return f as unknown as FilterState;
}

export function encodeView(v: ViewURL): string {
  const p = new URLSearchParams();
  if (v.mode && v.mode !== "typology") p.set("m", v.mode);
  const f = encodeFilter(v.filter);
  if (f) p.set("f", f);
  if (v.selected) p.set("s", v.selected);
  if (v.timeline !== null) p.set("t", String(v.timeline));
  if (v.camera) p.set("c", v.camera);
  const s = p.toString();
  return s ? `#${s}` : "";
}

export function decodeView(hash: string): ViewURL {
  const raw = hash.startsWith("#") ? hash.slice(1) : hash;
  const p = new URLSearchParams(raw);

  // A truncated link degrades to a sane view, not NaN coordinates.
  const str = (k: string): string | null => {
    const v = p.get(k);
    return v === null || v === "" ? null : v;
  };
  const num = (k: string): number | null => {
    const v = str(k);
    if (v === null) return null;
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  };

  const camera = str("c");
  const cameraOk =
    camera !== null &&
    camera.split(",").length === 5 &&
    camera.split(",").every((n) => Number.isFinite(Number(n)));

  return {
    mode: str("m") ?? "typology",
    filter: decodeFilter(p.get("f") ?? ""),
    selected: str("s"),
    timeline: num("t"),
    camera: cameraOk ? camera : null,
  };
}

export function isDefaultView(v: ViewURL): boolean {
  return (
    v.mode === "typology" &&
    !encodeFilter(v.filter) &&
    v.selected === null &&
    v.timeline === null &&
    v.camera === null
  );
}
