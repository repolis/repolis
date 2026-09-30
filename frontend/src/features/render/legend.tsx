import { useState } from "react";

import { LANGUAGE_COLORS, TYPOLOGY_COLORS, type CitySummary } from "./types";

/** Five visual channels are in use at once, and an encoding nobody can decode
 * is decoration. */
const MODE_HELP: Record<string, string> = {
  complexity:
    "Colour = worst cyclomatic complexity in the building. Pale is simple, red is branchy.",
  age: "Colour = recency. Red was edited recently, pale has not changed in a long time.",
  churn: "Colour = commit churn percentile. Red is edited constantly.",
  fanin:
    "Colour = how many other buildings depend on this one. Red is depended on heavily.",
  instability:
    "Colour = Martin's instability. Pale is depended upon, red depends on others.",
};

/** What each end of the ramp means, so the gradient is readable on its own. */
const RAMP_ENDS: Record<string, [string, string]> = {
  complexity: ["simple", "branchy"],
  age: ["long untouched", "edited recently"],
  churn: ["rarely edited", "edited constantly"],
  fanin: ["depended on by nothing", "depended on heavily"],
  instability: ["depended upon", "depends on others"],
};

function Swatch({ color, label }: { color: string; label: string }) {
  return (
    <span className="flex items-center gap-1 rounded bg-gray-800 px-1.5 py-0.5">
      <span
        className="inline-block h-2 w-2 rounded-sm"
        style={{ background: color }}
      />
      {label}
    </span>
  );
}

export function Legend({
  summary,
  mode = "typology",
}: {
  summary: CitySummary | null;
  mode?: string;
}) {
  const [open, setOpen] = useState(false);
  if (!summary) return null;

  const used = new Set(summary.districts.map((d) => d.typology));

  return (
    <div className="absolute right-4 bottom-4 z-40 w-64 rounded border border-gray-700 bg-gray-900/95 text-xs text-gray-200">
      <button
        className="flex w-full items-center justify-between px-3 py-2 text-left font-semibold"
        onClick={() => setOpen(!open)}
      >
        <span>Legend &amp; stats</span>
        <span className="text-gray-500">{open ? "−" : "+"}</span>
      </button>

      {open && (
        <div className="flex flex-col gap-3 border-t border-gray-800 p-3">
          <div className="flex flex-col gap-1">
            <div className="text-[10px] tracking-wide text-gray-500 uppercase">
              Shape
            </div>
            <div className="text-gray-300">Height = functions</div>
            <div className="text-gray-300">Footprint = fields</div>
            <div className="text-gray-300">Pale = not edited recently</div>
            <div className="flex items-center gap-1.5 text-gray-300">
              <span className="inline-block h-2 w-3 rounded-sm bg-orange-400" />
              Glowing roof cap = often edited
            </div>
            <div className="flex items-center gap-1.5 text-gray-300">
              <span className="inline-block h-1.5 w-3.5 rounded-sm bg-gray-600" />
              Dark plinth = file module, not a type
            </div>
            <div className="flex items-center gap-1.5 text-gray-300">
              <span className="inline-block h-0.5 w-3.5 rounded-sm bg-red-500" />
              Red arc = dependency cycle
            </div>
          </div>

          <div className="flex flex-col gap-1">
            {mode === "typology" ? (
              <>
                <div className="text-[10px] tracking-wide text-gray-500 uppercase">
                  Colour = district purpose
                </div>
                <div className="flex flex-wrap gap-1">
                  {Object.entries(TYPOLOGY_COLORS)
                    .filter(([t]) => used.has(t))
                    .map(([t, c]) => (
                      <Swatch key={t} color={c} label={t} />
                    ))}
                </div>
              </>
            ) : mode === "language" ? (
              <>
                <div className="text-[10px] tracking-wide text-gray-500 uppercase">
                  Colour = source language
                </div>
                <div className="flex flex-wrap gap-1">
                  {(summary.languages ?? []).map(([name, n]) => (
                    <Swatch
                      key={name}
                      color={LANGUAGE_COLORS[name] ?? LANGUAGE_COLORS.unknown}
                      label={`${name} ${n}`}
                    />
                  ))}
                </div>
              </>
            ) : (
              <>
                <div className="text-[10px] tracking-wide text-gray-500 uppercase">
                  Colour
                </div>
                <div className="text-gray-300">{MODE_HELP[mode]}</div>
                <div className="mt-0.5 h-2 w-full rounded-sm bg-gradient-to-r from-[rgb(219,217,199)] via-[rgb(217,112,56)] to-[rgb(184,41,43)]" />
                {RAMP_ENDS[mode] && (
                  <div className="flex justify-between text-[10px] text-gray-500">
                    <span>{RAMP_ENDS[mode][0]}</span>
                    <span>{RAMP_ENDS[mode][1]}</span>
                  </div>
                )}
              </>
            )}
          </div>

          <div className="flex flex-col gap-0.5 border-t border-gray-800 pt-2 text-[11px] text-gray-400">
            <div>
              {summary.total_buildings} entities ({summary.total_types} types,{" "}
              {summary.total_modules} modules)
            </div>
            <div>
              {summary.total_files} files &middot;{" "}
              {summary.total_loc.toLocaleString()} lines
            </div>
            {summary.languages?.length > 0 && (
              <div>
                {summary.languages
                  .map(([name, n]) => `${name} ${n}`)
                  .join(" \u00b7 ")}
              </div>
            )}
            <div>
              {summary.total_methods} functions attributed &middot;{" "}
              {summary.orphans} unattached
            </div>
            <div>
              by rule {summary.methods_by_rule} &middot; by model{" "}
              {summary.methods_by_llm} &middot; {summary.llm_calls} LLM calls
            </div>
            {summary.cycles?.length > 0 && (
              <div className="text-red-400">
                {summary.cycles.length} cross-module dependency{" "}
                {summary.cycles.length === 1 ? "cycle" : "cycles"} (
                {summary.cycles.map((c) => c.size).join(", ")} buildings)
              </div>
            )}
            {summary.skipped_dirs.length > 0 && (
              <div className="text-gray-500">
                skipped: {summary.skipped_dirs.slice(0, 3).join(", ")}
                {summary.skipped_dirs.length > 3
                  ? ` +${summary.skipped_dirs.length - 3}`
                  : ""}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
