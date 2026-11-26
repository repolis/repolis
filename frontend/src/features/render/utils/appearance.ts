import type { BuildingInfo, FilterState } from "../types";
import type { City } from "./city";
import { colorMode } from "./color-modes";
import { groundRgb } from "./colors";
import { isVisible } from "./filter";

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
