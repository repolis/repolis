import { useMemo, useState } from "react";

import type { CitySummary, IndexEntry } from "../types";
import { matchRank } from "../utils/city";
import { Regenerate } from "./regenerate";

export function SearchBar({
  summary,
  busy,
  onPick,
  onReset,
  onRegenerate,
}: {
  summary: CitySummary | null;
  busy: boolean;
  onPick: (id: string) => void;
  onReset: () => void;
  onRegenerate: (level: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);

  const results = useMemo<IndexEntry[]>(() => {
    if (!summary || query.trim().length < 2) return [];
    const lowerQuery = query.trim().toLowerCase();
    return summary.index
      .map((e) => ({ e, rank: matchRank(e.name, lowerQuery) }))
      .filter((r): r is { e: IndexEntry; rank: number } => r.rank !== null)
      .sort((a, b) => a.rank - b.rank || a.e.name.length - b.e.name.length)
      .slice(0, 12)
      .map((r) => r.e);
  }, [summary, query]);

  return (
    <div className="absolute top-4 left-4 z-50 w-[26rem] text-xs">
      <div className="flex gap-2">
        <input
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && results.length > 0) {
              onPick(results[0].id);
              setOpen(false);
            }
            if (e.key === "Escape") setOpen(false);
          }}
          placeholder="Search symbols..."
          className="flex-1 rounded border border-gray-700 bg-gray-900/95 px-2 py-1.5 text-gray-200 outline-none placeholder:text-gray-500 focus:border-gray-500"
        />
        <button
          onClick={onReset}
          title="Reset view"
          className="rounded border border-gray-700 bg-gray-900/95 px-2 py-1.5 text-gray-300 hover:border-gray-500"
        >
          Reset
        </button>
        <Regenerate busy={busy} onRun={onRegenerate} />
      </div>

      {open && results.length > 0 && (
        <div className="mt-1 max-h-72 w-72 overflow-y-auto rounded border border-gray-700 bg-gray-900/95">
          {results.map((r) => (
            <button
              key={r.id}
              className="flex w-full items-center justify-between gap-2 px-2 py-1.5 text-left hover:bg-gray-800"
              onClick={() => {
                onPick(r.id);
                setOpen(false);
              }}
            >
              <span className="truncate font-mono text-gray-200">{r.name}</span>
              <span className="shrink-0 text-[10px] text-gray-500">
                {r.district}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
