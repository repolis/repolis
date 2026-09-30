import { useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { spring } from "@/shared/lib/motion";
import { GlassButton } from "@/shared/ui/glass";
import {
  IconChevronDown,
  IconCross,
  IconFilter,
  IconPalette,
} from "@/shared/ui/icons";
import { MenuItem, Popover } from "@/shared/ui/popover";

import type { CitySummary } from "./types";

export const COLOR_MODES = [
  { id: "typology", label: "Purpose" },
  { id: "complexity", label: "Complexity" },
  { id: "age", label: "Recency" },
  { id: "churn", label: "Churn" },
  { id: "fanin", label: "Depended on" },
  { id: "instability", label: "Instability" },
  { id: "language", label: "Language" },
] as const;

const MODE_HINTS: Record<string, string> = {
  typology: "What each district is for",
  complexity: "Worst cyclomatic complexity",
  age: "How recently it was edited",
  churn: "How often it changes",
  fanin: "How much depends on it",
  instability: "Depended upon versus depending",
  language: "Source language",
};

export interface FilterState {
  text: string;
  language: string;
  district: string;
  kind: string;
  min_methods: number;
  min_complexity: number;
  min_churn_pct: number;
  only_no_callers: boolean;
  only_hubs: boolean;
  only_cycles: boolean;
}

export const EMPTY_FILTER: FilterState = {
  text: "",
  language: "",
  district: "",
  kind: "",
  min_methods: 0,
  min_complexity: 0,
  min_churn_pct: 0,
  only_no_callers: false,
  only_hubs: false,
  only_cycles: false,
};

function activeCount(f: FilterState): number {
  let n = 0;
  if (f.language) n++;
  if (f.district) n++;
  if (f.kind) n++;
  if (f.min_methods > 0) n++;
  if (f.min_complexity > 0) n++;
  if (f.min_churn_pct > 0) n++;
  if (f.only_no_callers) n++;
  if (f.only_hubs) n++;
  if (f.only_cycles) n++;
  return n;
}

/** Which metric drives building colour. */
export function ModeMenu({
  mode,
  onMode,
}: {
  mode: string;
  onMode: (m: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const current = COLOR_MODES.find((m) => m.id === mode) ?? COLOR_MODES[0];
  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      className="w-[18rem] max-md:fixed max-md:top-20 max-md:right-4 max-md:left-4 max-md:w-auto"
      trigger={
        <GlassButton
          size="lg"
          icon={<IconPalette />}
          onClick={() => setOpen(!open)}
          active={open}
          aria-haspopup="menu"
          aria-expanded={open}
          className="text-[1rem]"
        >
          <AnimatePresence mode="popLayout" initial={false}>
            <motion.span
              key={current.id}
              className="max-md:hidden"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={spring.snappy}
            >
              {current.label}
            </motion.span>
          </AnimatePresence>
          <motion.span
            animate={{ rotate: open ? 180 : 0 }}
            transition={spring.snappy}
            className="grid size-4 place-items-center text-white/60 max-md:hidden [&>svg]:size-full"
          >
            <IconChevronDown />
          </motion.span>
        </GlassButton>
      }
    >
      <div role="menu" className="flex flex-col">
        <div className="px-3.5 pt-2 pb-1.5 text-[0.75rem] font-semibold tracking-wide text-white/45 uppercase">
          Colour by
        </div>
        {COLOR_MODES.map((m) => (
          <MenuItem
            key={m.id}
            layoutGroup="mode"
            selected={m.id === mode}
            title={m.label}
            hint={MODE_HINTS[m.id]}
            onSelect={() => {
              onMode(m.id);
              setOpen(false);
            }}
          />
        ))}
      </div>
    </Popover>
  );
}

/** A toggle chip. Clicking sets the value, clicking again clears it. */
function Toggle({
  on,
  label,
  onClick,
}: {
  on: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <motion.button
      type="button"
      onClick={onClick}
      whileTap={{ scale: 0.94 }}
      transition={spring.snappy}
      className={cn(
        "relative rounded-full px-3 py-1.5 text-[0.8125rem] font-semibold transition-colors duration-200",
        on
          ? "bg-white text-[#0d0f14]"
          : "bg-white/10 text-white/75 shadow-[inset_0_1px_0_rgb(255_255_255/0.12)] hover:bg-white/18 hover:text-white",
      )}
    >
      {label}
    </motion.button>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-start gap-3">
      <span className="w-[5.5rem] shrink-0 pt-1.5 text-[0.8125rem] font-semibold text-white/50">
        {label}
      </span>
      <div className="flex flex-1 flex-wrap items-center gap-1.5">
        {children}
      </div>
    </div>
  );
}

function Slider({
  label,
  value,
  max,
  onChange,
  format,
}: {
  label: string;
  value: number;
  max: number;
  onChange: (v: number) => void;
  format: (v: number) => string;
}) {
  return (
    <Row label={label}>
      <input
        type="range"
        min={0}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="range-glass flex-1"
        style={{ ["--fill" as string]: `${(value / max) * 100}%` }}
      />
      <span className="w-16 text-right text-[0.8125rem] font-semibold text-white/70 tabular-nums">
        {format(value)}
      </span>
    </Row>
  );
}

/** Restricts the view. Buildings that do not match fade rather than vanish. */
export function FilterMenu({
  summary,
  filter,
  onFilter,
}: {
  summary: CitySummary;
  filter: FilterState;
  onFilter: (f: FilterState) => void;
}) {
  const [open, setOpen] = useState(false);
  const n = activeCount(filter);
  const set = (patch: Partial<FilterState>) =>
    onFilter({ ...filter, ...patch });
  const toggle = (k: keyof FilterState) =>
    set({ [k]: !filter[k] } as Partial<FilterState>);

  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      className="w-[26rem] p-5 max-md:fixed max-md:top-20 max-md:right-4 max-md:left-4 max-md:w-auto"
      trigger={
        <GlassButton
          size="lg"
          icon={<IconFilter />}
          onClick={() => setOpen(!open)}
          active={open}
          aria-haspopup="dialog"
          aria-expanded={open}
          className="text-[1rem]"
        >
          <span className="max-md:hidden">Filter</span>
          <AnimatePresence initial={false}>
            {n > 0 && (
              <motion.span
                initial={{ scale: 0, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0, opacity: 0 }}
                transition={spring.snappy}
                className="grid h-5 min-w-5 place-items-center rounded-full bg-[var(--color-signal)] px-1 text-[0.75rem] text-[#0d0f14] tabular-nums"
              >
                {n}
              </motion.span>
            )}
          </AnimatePresence>
        </GlassButton>
      }
    >
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between">
          <span className="text-[1rem] font-semibold text-white">
            Filter the city
          </span>
          {n > 0 && (
            <button
              type="button"
              onClick={() => onFilter(EMPTY_FILTER)}
              className="flex items-center gap-1 text-[0.8125rem] font-semibold text-white/55 hover:text-white"
            >
              <IconCross className="size-4" />
              Clear all
            </button>
          )}
        </div>

        <Row label="Shape">
          <Toggle
            on={filter.kind === "type"}
            label="types"
            onClick={() => set({ kind: filter.kind === "type" ? "" : "type" })}
          />
          <Toggle
            on={filter.kind === "module"}
            label="modules"
            onClick={() =>
              set({ kind: filter.kind === "module" ? "" : "module" })
            }
          />
        </Row>

        {summary.languages?.length > 1 && (
          <Row label="Language">
            {summary.languages.map(([name]) => (
              <Toggle
                key={name}
                on={filter.language === name}
                label={name}
                onClick={() =>
                  set({ language: filter.language === name ? "" : name })
                }
              />
            ))}
          </Row>
        )}

        <Row label="Graph">
          <Toggle
            on={filter.only_hubs}
            label="hubs"
            onClick={() => toggle("only_hubs")}
          />
          <Toggle
            on={filter.only_no_callers}
            label="no callers"
            onClick={() => toggle("only_no_callers")}
          />
          <Toggle
            on={filter.only_cycles}
            label="in a cycle"
            onClick={() => toggle("only_cycles")}
          />
        </Row>

        <Slider
          label="Complexity"
          value={filter.min_complexity}
          max={40}
          onChange={(v) => set({ min_complexity: v })}
          format={(v) => (v > 0 ? `≥ ${v}` : "any")}
        />
        <Slider
          label="Churn"
          value={filter.min_churn_pct}
          max={99}
          onChange={(v) => set({ min_churn_pct: v })}
          format={(v) => (v > 0 ? `top ${100 - v}%` : "any")}
        />
        <Slider
          label="Functions"
          value={filter.min_methods}
          max={40}
          onChange={(v) => set({ min_methods: v })}
          format={(v) => (v > 0 ? `≥ ${v}` : "any")}
        />

        <p className="border-t border-white/10 pt-3 text-[0.75rem] leading-snug text-white/45">
          Buildings that do not match fade out rather than disappear, so a match
          is still read in context. &quot;No callers&quot; is a question, not a
          verdict: a library&apos;s whole public surface has none.
        </p>
      </div>
    </Popover>
  );
}
