import { useState, type ReactNode } from "react";

import { formatCount } from "@/shared/lib/format";
import { GlassButton } from "@/shared/ui/glass";
import { IconLayers } from "@/shared/ui/icons";
import { Popover } from "@/shared/ui/popover";

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
    <span className="flex items-center gap-1.5 rounded-full bg-white/10 px-3 py-1.5 text-[0.9375rem] text-white/80">
      <span className="size-2 rounded-full" style={{ background: color }} />
      {label}
    </span>
  );
}

function Heading({ children }: { children: ReactNode }) {
  return <div className="text-[0.9375rem] text-white/45">{children}</div>;
}

function Key({ mark, children }: { mark: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center gap-2.5 text-[0.9375rem] text-white/80">
      <span className="grid w-5 shrink-0 place-items-center">{mark}</span>
      {children}
    </div>
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
    <div className="hud-dim absolute right-10 bottom-10 z-[45] max-md:right-4 max-md:bottom-4">
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        side="top"
        align="right"
        className="w-[26rem] p-5 max-md:w-[calc(100vw-2rem)]"
        trigger={
          <GlassButton
            icon={<IconLayers />}
            onClick={() => setOpen(!open)}
            active={open}
            aria-expanded={open}
          >
            <span className="max-md:hidden">Legend</span>
          </GlassButton>
        }
      >
        <div className="scrollbar-glass flex max-h-[calc(100vh-14rem)] flex-col gap-5 overflow-y-auto">
          <div className="flex flex-col gap-2">
            <Heading>Shape</Heading>
            <Key
              mark={
                <span className="flex items-end gap-0.5">
                  <span className="h-2 w-1 rounded-sm bg-white/60" />
                  <span className="h-3.5 w-1 rounded-sm bg-white/80" />
                </span>
              }
            >
              Height = functions
            </Key>
            <Key mark={<span className="h-2 w-4 rounded-sm bg-white/60" />}>
              Footprint = fields
            </Key>
            <Key mark={<span className="size-3 rounded-sm bg-white/25" />}>
              Pale = not edited recently
            </Key>
            <Key mark={<span className="h-2 w-3.5 rounded-sm bg-orange-400" />}>
              Glowing roof cap = often edited
            </Key>
            <Key
              mark={
                <span className="h-1.5 w-4 rounded-sm bg-black ring-1 ring-white/25" />
              }
            >
              Dark plinth = file module, not a type
            </Key>
            <Key mark={<span className="h-0.5 w-4 rounded-full bg-red-400" />}>
              Red arc = dependency cycle
            </Key>
          </div>

          <div className="flex flex-col gap-2">
            {mode === "typology" ? (
              <>
                <Heading>Colour = district purpose</Heading>
                <div className="flex flex-wrap gap-1.5">
                  {Object.entries(TYPOLOGY_COLORS)
                    .filter(([t]) => used.has(t))
                    .map(([t, c]) => (
                      <Swatch key={t} color={c} label={t} />
                    ))}
                </div>
              </>
            ) : mode === "language" ? (
              <>
                <Heading>Colour = source language</Heading>
                <div className="flex flex-wrap gap-1.5">
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
                <Heading>Colour</Heading>
                <p className="text-[0.9375rem] leading-snug font-medium text-white/75">
                  {MODE_HELP[mode]}
                </p>
                <div className="mt-1 h-2 w-full rounded-full bg-gradient-to-r from-[rgb(219,217,199)] via-[rgb(217,112,56)] to-[rgb(184,41,43)]" />
                {RAMP_ENDS[mode] && (
                  <div className="flex justify-between text-[0.875rem] text-white/50">
                    <span>{RAMP_ENDS[mode][0]}</span>
                    <span>{RAMP_ENDS[mode][1]}</span>
                  </div>
                )}
              </>
            )}
          </div>

          <div className="flex flex-col gap-1 border-t border-white/10 pt-4 text-[0.9375rem] leading-relaxed font-medium text-white/60">
            <div>
              <b className="text-white/85">
                {formatCount(summary.total_buildings)}
              </b>{" "}
              buildings ({formatCount(summary.total_types)} types,{" "}
              {formatCount(summary.total_modules)} modules)
            </div>
            <div>
              <b className="text-white/85">
                {formatCount(summary.total_files)}
              </b>{" "}
              files &middot; {formatCount(summary.total_loc)} lines
            </div>
            {summary.languages?.length > 0 && (
              <div>
                {summary.languages
                  .map(([name, n]) => `${name} ${n}`)
                  .join(" · ")}
              </div>
            )}
            <div>
              {formatCount(summary.total_methods)} functions attributed &middot;{" "}
              {formatCount(summary.orphans)} unattached
            </div>
            <div>
              by rule {formatCount(summary.methods_by_rule)} &middot; by model{" "}
              {formatCount(summary.methods_by_llm)} &middot; {summary.llm_calls}{" "}
              LLM calls
            </div>
            {summary.cycles?.length > 0 && (
              <div className="text-[var(--color-danger)]">
                {summary.cycles.length} cross-module dependency{" "}
                {summary.cycles.length === 1 ? "cycle" : "cycles"} (
                {summary.cycles.map((c) => c.size).join(", ")} buildings)
              </div>
            )}
            {summary.skipped_dirs.length > 0 && (
              <div className="text-white/40">
                skipped: {summary.skipped_dirs.slice(0, 3).join(", ")}
                {summary.skipped_dirs.length > 3
                  ? ` +${summary.skipped_dirs.length - 3}`
                  : ""}
              </div>
            )}
          </div>
        </div>
      </Popover>
    </div>
  );
}
