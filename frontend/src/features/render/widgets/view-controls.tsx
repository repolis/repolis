import { useState, type ReactNode } from "react";

import type { CitySummary, FilterState } from "../types";
import { COLOR_MODES, EMPTY_FILTER } from "../utils/appearance";

function activeCount(f: FilterState): number {
  return (Object.keys(EMPTY_FILTER) as (keyof FilterState)[]).filter(
    (k) => k !== "text" && f[k] !== EMPTY_FILTER[k],
  ).length;
}

interface ChipProps {
  on: boolean;
  label: string;
  onClick: () => void;
}

function Chip({ on, label, onClick }: ChipProps) {
  return (
    <button
      onClick={onClick}
      className={`rounded border px-1.5 py-0.5 ${
        on
          ? "border-gray-400 bg-gray-700 text-white"
          : "border-gray-700 text-gray-300 hover:border-gray-500"
      }`}
    >
      {label}
    </button>
  );
}

interface Props {
  summary: CitySummary | null;
  mode: string;
  filter: FilterState;
  links: boolean;
  table: boolean;
  onMode: (m: string) => void;
  onFilter: (f: FilterState) => void;
  onLinks: (on: boolean) => void;
  onTable: (on: boolean) => void;
  children?: ReactNode;
}

export function ViewControls({
  summary,
  mode,
  filter,
  links,
  table,
  onMode,
  onFilter,
  onLinks,
  onTable,
  children,
}: Props) {
  const [open, setOpen] = useState(false);
  if (!summary) return null;

  const n = activeCount(filter);
  const set = (patch: Partial<FilterState>) =>
    onFilter({ ...filter, ...patch });
  const toggle = (k: keyof FilterState) =>
    set({ [k]: !filter[k] } as Partial<FilterState>);

  return (
    <div className="absolute top-14 left-4 z-40 w-[26rem] text-xs">
      <div className="flex items-center gap-2 rounded border border-gray-700 bg-gray-900/95 px-2 py-1.5">
        <span className="text-gray-500">Colour</span>
        <select
          value={mode}
          onChange={(e) => onMode(e.target.value)}
          className="flex-1 rounded border border-gray-700 bg-gray-900 px-1 py-0.5 text-gray-200 outline-none"
        >
          {COLOR_MODES.map((m) => (
            <option key={m.id} value={m.id}>
              {m.label}
            </option>
          ))}
        </select>
        <button
          onClick={() => setOpen(!open)}
          className="rounded border border-gray-700 px-1.5 py-0.5 text-gray-300 hover:border-gray-500"
        >
          Filter{n > 0 ? ` (${n})` : ""}
        </button>
        {n > 0 && (
          <button
            onClick={() => onFilter(EMPTY_FILTER)}
            className="text-gray-500 hover:text-gray-200"
            title="Clear all filters"
          >
            x
          </button>
        )}
        <Chip on={links} label="Links" onClick={() => onLinks(!links)} />
        <Chip on={table} label="Table" onClick={() => onTable(!table)} />
      </div>

      {open && (
        <div className="mt-1 flex flex-col gap-2 rounded border border-gray-700 bg-gray-900/95 p-2.5">
          <div className="flex flex-wrap items-center gap-1">
            <span className="w-16 text-gray-500">Shape</span>
            <Chip
              on={filter.kind === "type"}
              label="types"
              onClick={() =>
                set({ kind: filter.kind === "type" ? "" : "type" })
              }
            />
            <Chip
              on={filter.kind === "module"}
              label="modules"
              onClick={() =>
                set({ kind: filter.kind === "module" ? "" : "module" })
              }
            />
          </div>

          {summary.languages?.length > 1 && (
            <div className="flex flex-wrap items-center gap-1">
              <span className="w-16 text-gray-500">Language</span>
              {summary.languages.map(([name]) => (
                <Chip
                  key={name}
                  on={filter.language === name}
                  label={name}
                  onClick={() =>
                    set({ language: filter.language === name ? "" : name })
                  }
                />
              ))}
            </div>
          )}

          <div className="flex flex-wrap items-center gap-1">
            <span className="w-16 text-gray-500">Graph</span>
            <Chip
              on={filter.only_hubs}
              label="hubs"
              onClick={() => toggle("only_hubs")}
            />
            <Chip
              on={filter.only_no_callers}
              label="no callers"
              onClick={() => toggle("only_no_callers")}
            />
            <Chip
              on={filter.only_cycles}
              label="in a cycle"
              onClick={() => toggle("only_cycles")}
            />
          </div>

          <div className="flex items-center gap-2">
            <span className="w-16 shrink-0 text-gray-500">Complexity</span>
            <input
              type="range"
              min={0}
              max={40}
              value={filter.min_complexity}
              onChange={(e) => set({ min_complexity: Number(e.target.value) })}
              className="flex-1"
            />
            <span className="w-10 text-right text-gray-400">
              {filter.min_complexity > 0 ? `>=${filter.min_complexity}` : "any"}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="w-16 shrink-0 text-gray-500">Churn</span>
            <input
              type="range"
              min={0}
              max={99}
              value={filter.min_churn_pct}
              onChange={(e) => set({ min_churn_pct: Number(e.target.value) })}
              className="flex-1"
            />
            <span className="w-10 text-right text-gray-400">
              {filter.min_churn_pct > 0
                ? `top ${100 - filter.min_churn_pct}%`
                : "any"}
            </span>
          </div>

          <div className="flex items-center gap-2">
            <span className="w-16 shrink-0 text-gray-500">Functions</span>
            <input
              type="range"
              min={0}
              max={40}
              value={filter.min_methods}
              onChange={(e) => set({ min_methods: Number(e.target.value) })}
              className="flex-1"
            />
            <span className="w-10 text-right text-gray-400">
              {filter.min_methods > 0 ? `>=${filter.min_methods}` : "any"}
            </span>
          </div>

          <div className="border-t border-gray-800 pt-1.5 text-[10px] leading-snug text-gray-500">
            Buildings that do not match fade out rather than disappear, so a
            match is still read in context. &quot;No callers&quot; is a
            question, not a verdict: a library&apos;s whole public surface has
            none.
          </div>
        </div>
      )}
      {children}
    </div>
  );
}
