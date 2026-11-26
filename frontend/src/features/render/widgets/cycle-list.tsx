import type { CycleInfo } from "../types";
import { closedLoop } from "../utils/appearance";

interface Props {
  cycles: CycleInfo[];
  active: number | null;
  loops: string[][];
  loop: string[] | null;
  nameOf: (id: string) => string;
  onCycle: (id: number) => void;
  onLoop: (loop: string[]) => void;
}

export function CycleList({
  cycles,
  active,
  loops,
  loop,
  nameOf,
  onCycle,
  onLoop,
}: Props) {
  if (cycles.length === 0) return null;

  return (
    <div className="absolute bottom-10 left-3 z-40 flex max-h-[60vh] w-80 flex-col overflow-y-auto rounded border border-gray-700 bg-gray-900/95 text-xs text-gray-200">
      <div className="sticky top-0 bg-gray-900 px-3 py-2 font-semibold">
        Dependency cycles ({cycles.length})
      </div>
      {cycles.map((c) => (
        <div key={c.id} className="border-t border-gray-800">
          <button
            onClick={() => onCycle(c.id)}
            className={`w-full px-3 py-1.5 text-left hover:bg-gray-800 ${c.id === active ? "bg-gray-800" : ""}`}
          >
            <div className="text-red-400">
              Cycle {c.id}: {c.size} buildings, {c.namespaces} modules
            </div>
            {c.id !== active && (
              <div className="truncate font-mono text-[10px] text-gray-400">
                {c.members.map(nameOf).join(", ")}
              </div>
            )}
          </button>

          {c.id === active &&
            loops.map((l, i) => (
              <button
                key={i}
                onClick={() => onLoop(l)}
                className={`block w-full px-3 py-1 text-left font-mono text-[10px] hover:bg-gray-800 ${l === loop ? "text-white" : "text-gray-400"}`}
              >
                {l === loop ? (
                  closedLoop(l).map((id, k) => (
                    <div key={k} className="break-all">
                      {k > 0 && <span className="text-red-400">{"-> "}</span>}
                      {nameOf(id)}
                    </div>
                  ))
                ) : (
                  <div className="truncate">
                    {closedLoop(l).map(nameOf).join(" -> ")}
                  </div>
                )}
              </button>
            ))}
        </div>
      ))}
    </div>
  );
}
