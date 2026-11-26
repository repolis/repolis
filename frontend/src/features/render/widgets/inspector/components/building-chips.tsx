import { useState } from "react";

import type { BuildingRef } from "../../../types";

interface Props {
  refs: BuildingRef[];
  limit: number;
  onPick: (id: string) => void;
}

export function BuildingChips({ refs, limit, onPick }: Props) {
  const [expanded, setExpanded] = useState(false);
  const shown = expanded ? refs : refs.slice(0, limit);

  return (
    <div className="flex min-w-0 flex-wrap gap-1">
      {shown.map((ref, i) => (
        <button
          key={`${ref.id}-${i}`}
          onClick={() => onPick(ref.id)}
          title={ref.file}
          className="max-w-full rounded bg-gray-800 px-1 py-0.5 text-left font-mono text-[10px] break-all text-gray-300 hover:bg-gray-700 hover:text-white"
        >
          {ref.name}
        </button>
      ))}
      {refs.length > limit && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-[10px] text-gray-500 hover:text-gray-300"
        >
          {expanded ? "show less" : `+${refs.length - limit} more`}
        </button>
      )}
    </div>
  );
}
