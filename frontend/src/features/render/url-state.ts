import { EMPTY_FILTER, type FilterState } from "./view-controls";

/**
 * The part of the view worth putting in a link. Encoded in the fragment so it
 * never reaches the server, and written with replaceState so panning does not
 * fill the back button.
 */
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

/** Compact filter encoding: only what differs from the default is written. */
function encodeFilter(f: FilterState): string {
  const parts: string[] = [];
  if (f.text) parts.push(`q:${f.text}`);
  if (f.language) parts.push(`lang:${f.language}`);
  if (f.district) parts.push(`dist:${f.district}`);
  if (f.kind) parts.push(`kind:${f.kind}`);
  if (f.min_methods > 0) parts.push(`fn:${f.min_methods}`);
  if (f.min_complexity > 0) parts.push(`cx:${f.min_complexity}`);
  if (f.min_churn_pct > 0) parts.push(`churn:${f.min_churn_pct}`);
  if (f.only_no_callers) parts.push("nocallers");
  if (f.only_hubs) parts.push("hubs");
  if (f.only_cycles) parts.push("cycles");
  return parts.join(";");
}

function clampNum(v: string): number {
  const n = Number(v);
  return Number.isFinite(n) && n > 0 ? n : 0;
}

function decodeFilter(s: string): FilterState {
  const f: FilterState = { ...EMPTY_FILTER };
  if (!s) return f;
  for (const part of s.split(";")) {
    const [k, ...rest] = part.split(":");
    const v = rest.join(":");
    switch (k) {
      case "q":
        f.text = v;
        break;
      case "lang":
        f.language = v;
        break;
      case "dist":
        f.district = v;
        break;
      case "kind":
        f.kind = v;
        break;
      case "fn":
        f.min_methods = clampNum(v);
        break;
      case "cx":
        f.min_complexity = clampNum(v);
        break;
      case "churn":
        f.min_churn_pct = clampNum(v);
        break;
      case "nocallers":
        f.only_no_callers = true;
        break;
      case "hubs":
        f.only_hubs = true;
        break;
      case "cycles":
        f.only_cycles = true;
        break;
    }
  }
  return f;
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

/** True when a decoded view carries nothing worth restoring. */
export function isDefaultView(v: ViewURL): boolean {
  return (
    v.mode === "typology" &&
    !encodeFilter(v.filter) &&
    v.selected === null &&
    v.timeline === null &&
    v.camera === null
  );
}
