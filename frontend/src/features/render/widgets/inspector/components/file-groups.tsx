import { useMemo, useState } from "react";

import type { BuildingRef } from "../../../types";
import { groupByFile } from "../../../utils/file-groups";
import { BuildingChips } from "./building-chips";

const FILES_SHOWN = 5;
const CHIPS_PER_FILE = 6;

interface Props {
  refs: BuildingRef[];
  onPick: (id: string) => void;
}

export function FileGroups({ refs, onPick }: Props) {
  const [expanded, setExpanded] = useState(false);
  const groups = useMemo(() => groupByFile(refs), [refs]);
  const shown = expanded ? groups : groups.slice(0, FILES_SHOWN);

  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      {shown.map((g) => (
        <div key={g.file} className="flex min-w-0 flex-col gap-0.5">
          <div className="font-mono text-[10px] break-all text-gray-500">
            {g.file || "unknown file"}
          </div>
          <BuildingChips refs={g.refs} limit={CHIPS_PER_FILE} onPick={onPick} />
        </div>
      ))}
      {groups.length > FILES_SHOWN && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="self-start text-[10px] text-gray-500 hover:text-gray-300"
        >
          {expanded
            ? "show less"
            : `+${groups.length - FILES_SHOWN} more files`}
        </button>
      )}
    </div>
  );
}
