import type { BuildingInfo, CitySummary } from "../types";
import {
  applyAge,
  clamp01,
  GREY,
  LANGUAGE_COLORS,
  LANGUAGE_RGB,
  NO_SMELL,
  ramp,
  SMELL_COLORS,
  SMELL_LABELS,
  SMELL_RGB,
  TYPOLOGY_COLORS,
  TYPOLOGY_RGB,
  type Rgb,
} from "./colors";

const RECENCY_HORIZON_DAYS = 540;

export interface Scales {
  complexity: number;
  fanIn: number;
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
    born: Math.max(1, ...buildings.map((b) => b.born_day)),
  };
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
    color: (b) => (b.smell ? SMELL_RGB[b.smell] : NO_SMELL),
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
