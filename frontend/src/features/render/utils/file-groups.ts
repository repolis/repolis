import type { BuildingRef } from "../types";

export interface FileGroup {
  file: string;
  refs: BuildingRef[];
}

// Files with the most links first: they are where a change would spread.
export function groupByFile(refs: BuildingRef[]): FileGroup[] {
  const byFile = new Map<string, BuildingRef[]>();
  for (const ref of refs) {
    const group = byFile.get(ref.file);
    if (group) group.push(ref);
    else byFile.set(ref.file, [ref]);
  }
  return [...byFile.entries()]
    .map(([file, group]) => ({
      file,
      refs: group.sort((a, b) => a.name.localeCompare(b.name)),
    }))
    .sort(
      (a, b) => b.refs.length - a.refs.length || a.file.localeCompare(b.file),
    );
}
