import type { Smell } from "../types";

export type Rgb = [number, number, number];

export const GREY: Rgb = [0.62, 0.62, 0.64];

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

export const SMELL_RGB: Record<Smell, Rgb> = {
  god: [0.82, 0.2, 0.22],
  brain: [0.9, 0.6, 0.2],
  data: [0.35, 0.55, 0.85],
};

export const SMELL_LABELS: Record<Smell, string> = {
  god: "god class",
  brain: "very complex function",
  data: "data-only struct",
};

export const NO_SMELL: Rgb = [0.3, 0.3, 0.32];

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
export const SMELL_COLORS = cssTable(SMELL_RGB);
export const RAMP_CSS = `linear-gradient(to right, ${RAMP_STOPS.map(css).join(", ")})`;

export const clamp01 = (v: number) => Math.min(Math.max(v, 0), 1);

export function ramp(t: number): Rgb {
  const position = clamp01(t) * (RAMP_STOPS.length - 1);
  const segment = Math.floor(position);
  const next = Math.min(segment + 1, RAMP_STOPS.length - 1);
  const within = position - segment;
  return RAMP_STOPS[segment].map(
    (c, channel) => c + (RAMP_STOPS[next][channel] - c) * within,
  ) as Rgb;
}

/** Buckets, not a smooth ramp: they compare better and keep the engine's materials bounded. */
const AGE_BUCKETS = [45, 180, 400, 900];

export function applyAge(rgb: Rgb, ageDays: number): Rgb {
  const bucketsPassed = AGE_BUCKETS.filter((edge) => ageDays >= edge).length;
  const greyMix = 0.55 * (bucketsPassed / AGE_BUCKETS.length);
  const grey = (rgb[0] + rgb[1] + rgb[2]) / 3;
  return rgb.map((c) => c * (1 - greyMix) + grey * greyMix) as Rgb;
}

export const groundRgb = (typology: string): Rgb => {
  const [r, g, b] = TYPOLOGY_RGB[typology] ?? GREY;
  return [r * 0.16 + 0.05, g * 0.16 + 0.05, b * 0.16 + 0.06];
};
