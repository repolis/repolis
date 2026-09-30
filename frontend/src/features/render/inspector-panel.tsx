import { useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";

import { GLASS } from "@/shared/glass/maps";
import { cn } from "@/shared/lib/cn";
import { formatAge, formatCount } from "@/shared/lib/format";
import { spring } from "@/shared/lib/motion";
import { AutoHeight } from "@/shared/ui/auto-height";
import { Shimmer } from "@/shared/ui/brand";
import { Chip, Glass, StatPill } from "@/shared/ui/glass";
import {
  IconBranch,
  IconChevronDown,
  IconCity,
  IconCode,
  IconCross,
  IconFunction,
  IconHashtag,
  IconMagic,
  IconRouting,
  IconTarget,
} from "@/shared/ui/icons";

import {
  TYPOLOGY_COLORS,
  type BuildingInfo,
  type DistrictInfo,
  type SelectPayload,
} from "./types";

interface Props {
  selection: SelectPayload;
  repoUrl: string;
  onClose: () => void;
  onPick: (nameOrId: string) => void;
  /** Pin this building as the start of a path query. */
  onAnchor: (buildingId: string) => void;
  /** Trace from the pinned start to this building. */
  onPath: (buildingId: string) => void;
  /** Id of the currently pinned start, if any. */
  anchorId: string | null;
}

/** Pinned inspector: appears on click, stays until dismissed. Laid out as the
 * Figma card: a counter, a bright lead sentence running into quieter text,
 * then 15% pills for the numbers. Longer lists fold away beneath. */
export function InspectorPanel({
  selection,
  repoUrl,
  onClose,
  onPick,
  onAnchor,
  onPath,
  anchorId,
}: Props) {
  const key =
    selection.type === "None"
      ? "none"
      : `${selection.type}:${selection.data.id}`;

  return (
    <AnimatePresence>
      {selection.type !== "None" && (
        <Glass
          key="inspector"
          params={GLASS.card}
          radius={24}
          fill="rgb(255 255 255 / 0.01)"
          className="inspector-pos hud-dim absolute right-10 z-40 flex w-[27.25rem] flex-col max-lg:right-5 max-sm:inset-x-3 max-sm:bottom-3 max-sm:w-auto"
          initial={{ opacity: 0, x: 32 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 24 }}
          transition={spring.glass}
        >
          <motion.button
            type="button"
            onClick={onClose}
            title="Close (Esc)"
            aria-label="Close inspector"
            whileHover={{ rotate: 90 }}
            whileTap={{ scale: 0.88 }}
            transition={spring.snappy}
            className="absolute top-[0.9375rem] right-[0.9375rem] z-10 grid size-8 place-items-center rounded-full opacity-100 transition-opacity hover:opacity-80"
          >
            <IconCross className="size-8" />
          </motion.button>

          <AutoHeight
            className="scrollbar-glass relative min-h-0 overflow-x-hidden overflow-y-auto"
            innerClassName="px-8 pt-8 pb-9"
          >
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={key}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={spring.glass}
              >
                {selection.type === "Building" && (
                  <BuildingInspector
                    key={selection.data.id}
                    info={selection.data}
                    repoUrl={repoUrl}
                    onPick={onPick}
                    onAnchor={onAnchor}
                    onPath={onPath}
                    anchorId={anchorId}
                  />
                )}
                {selection.type === "District" && (
                  <DistrictInspector info={selection.data} onPick={onPick} />
                )}
              </motion.div>
            </AnimatePresence>
          </AutoHeight>
        </Glass>
      )}
    </AnimatePresence>
  );
}

/** Figma header: icon, white count, quieter noun; 8px apart. */
function Counter({
  icon,
  value,
  label,
}: {
  icon: ReactNode;
  value: number;
  label: string;
}) {
  return (
    <div className="type-hud flex items-center gap-2 pr-8">
      <span className="grid size-[1.375rem] place-items-center [&>svg]:size-full">
        {icon}
      </span>
      <span>
        <span className="text-white tabular-nums">{formatCount(value)}</span>
        <span className="text-white/85"> </span>
        <span className="text-white/65">{label}</span>
      </span>
    </div>
  );
}

/** Figma body text: 20px, a Semibold white lead into Regular at 65%. */
function Lead({ lead, rest }: { lead: ReactNode; rest?: ReactNode }) {
  return (
    <p className="type-hud mt-4 break-words text-white">
      {lead}
      {rest && (
        <>
          {" "}
          <span className="font-normal text-white/65">{rest}</span>
        </>
      )}
    </p>
  );
}

function splitLead(text: string): [string, string] {
  const m = text.match(/^(.+?[.!?])(\s+)([\s\S]*)$/);
  return m ? [m[1], m[3]] : [text, ""];
}

function Pills({ children }: { children: ReactNode }) {
  return <div className="mt-8 flex flex-wrap gap-2">{children}</div>;
}

function Fold({
  title,
  count,
  children,
}: {
  title: string;
  count?: number;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <div className="border-t border-white/10">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="group flex w-full items-center gap-2 py-3.5 text-left text-[1.0625rem] text-white/85 transition-colors duration-300 hover:text-white"
      >
        <span className="flex-1">{title}</span>
        {count !== undefined && (
          <span className="text-white/45 tabular-nums">{count}</span>
        )}
        <motion.span
          animate={{ rotate: open ? 180 : 0 }}
          transition={spring.snappy}
          className="grid size-4 place-items-center text-white/45 transition-colors duration-300 group-hover:text-white/85 [&>svg]:size-full"
        >
          <IconChevronDown />
        </motion.span>
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={spring.glass}
            className="overflow-hidden"
          >
            <div className="pb-4">{children}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function Chips({
  items,
  limit,
  onPick,
}: {
  items: string[];
  limit: number;
  onPick?: (n: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const shown = expanded ? items : items.slice(0, limit);
  return (
    <div className="flex min-w-0 flex-wrap gap-1.5">
      {shown.map((v, i) => (
        <Chip key={i} mono onClick={onPick ? () => onPick(v) : undefined}>
          {v}
        </Chip>
      ))}
      {items.length > limit && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="rounded-full px-3 py-1.5 text-[0.9375rem] text-white/55 hover:text-white"
        >
          {expanded ? "show less" : `+${items.length - limit} more`}
        </button>
      )}
    </div>
  );
}

/** A 15% pill that acts, in the same shape as the numbers above it. */
function Action({
  icon,
  children,
  onClick,
  disabled,
  strong,
}: {
  icon: ReactNode;
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  strong?: boolean;
}) {
  return (
    <motion.button
      type="button"
      onClick={onClick}
      disabled={disabled}
      whileTap={{ scale: 0.97 }}
      transition={spring.snappy}
      className={cn(
        "glass-control type-hud inline-flex h-12 items-center gap-2 rounded-full px-[1.125rem] transition-colors duration-300 ease-[var(--ease-glass)] disabled:opacity-60",
        strong
          ? "bg-white text-[#0d0f14] hover:bg-white/90"
          : "bg-white/15 text-white/85 hover:bg-white/20 hover:text-white",
      )}
    >
      <span className="glass-icon grid size-[1.375rem] place-items-center [&>svg]:size-full">
        {icon}
      </span>
      {children}
    </motion.button>
  );
}

function BuildingInspector({
  info,
  repoUrl,
  onPick,
  onAnchor,
  onPath,
  anchorId,
}: {
  info: BuildingInfo;
  repoUrl: string;
  onPick: (n: string) => void;
  onAnchor: (id: string) => void;
  onPath: (id: string) => void;
  anchorId: string | null;
}) {
  const [explanation, setExplanation] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // The larger model runs here and only here; nothing in the render path
  // waits on it.
  async function explain() {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/explain", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ repo_url: repoUrl, building_id: info.id }),
      });
      const data = await res.json();
      if (data.explanation) setExplanation(data.explanation);
      else setError(data.error ?? "Could not generate an explanation");
    } catch {
      setError("Could not reach the server");
    } finally {
      setLoading(false);
    }
  }

  const summary = explanation ?? info.summary;
  const [lead, rest] = summary ? splitLead(summary) : ["", ""];
  const facts = [
    info.kind === "module" ? "File module" : "Type",
    info.language,
    info.district_name && `in ${info.district_name}`,
    info.hub && "a hub",
  ]
    .filter(Boolean)
    .join(" · ");
  const history = [
    `${info.commit_churn} commits`,
    `last edited ${formatAge(info.age_days)}`,
    info.primary_author && `mostly by ${info.primary_author}`,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <div className="flex min-w-0 flex-col">
      <Counter
        icon={<IconFunction />}
        value={info.num_methods}
        label={info.num_methods === 1 ? "function" : "functions"}
      />

      <Lead
        lead={<span className="font-mono">{info.name}.</span>}
        rest={
          <>
            {facts}. {info.source_file && <>{info.source_file}. </>}
            {history}.
            {info.cycle_id > 0 && (
              <span className="text-[var(--color-danger)]">
                {" "}
                Part of dependency cycle {info.cycle_id}.
              </span>
            )}
            {info.assoc_source === "llm" && (
              <span className="text-[var(--color-amber)]">
                {" "}
                Some functions were attributed by the model.
              </span>
            )}
          </>
        }
      />

      {summary && <Lead lead={lead} rest={rest} />}
      {error && (
        <p className="mt-3 text-[1.0625rem] text-[var(--color-danger)]">
          {error}
        </p>
      )}

      <Pills>
        <StatPill
          tone="inset"
          icon={<IconCode />}
          value={formatCount(info.lines_of_code)}
          label="lines of code"
        />
        <StatPill
          tone="inset"
          icon={<IconHashtag />}
          value={formatCount(info.num_fields)}
          label={info.num_fields === 1 ? "field" : "fields"}
        />
        <StatPill
          tone="inset"
          icon={<IconTarget />}
          value={formatCount(info.fan_in)}
          label="used by"
        />
        <StatPill
          tone="inset"
          icon={<IconBranch />}
          value={formatCount(info.fan_out)}
          label="uses"
        />
      </Pills>

      <div className="mt-2 flex flex-wrap gap-2">
        {!summary && (
          <Action icon={<IconMagic />} onClick={explain} disabled={loading}>
            {loading ? <Shimmer>Reading the source…</Shimmer> : "Explain"}
          </Action>
        )}
        {anchorId === null ? (
          <Action icon={<IconRouting />} onClick={() => onAnchor(info.id)}>
            Start a path
          </Action>
        ) : anchorId === info.id ? (
          <Action icon={<IconRouting />} onClick={() => onAnchor("")}>
            Cancel path
          </Action>
        ) : (
          <Action strong icon={<IconRouting />} onClick={() => onPath(info.id)}>
            Trace path here
          </Action>
        )}
      </div>

      <div className="mt-6">
        <Fold title="Complexity">
          <p className="text-[1.0625rem] font-normal text-white/65">
            Worst function{" "}
            <span className="font-semibold text-white">
              {info.max_complexity > 0 ? info.max_complexity : "–"}
            </span>
            , total{" "}
            <span className="font-semibold text-white">
              {info.sum_complexity}
            </span>
            , instability{" "}
            <span className="font-semibold text-white">
              {info.instability.toFixed(2)}
            </span>
            .
          </p>
        </Fold>
        {info.methods.length > 0 && (
          <Fold title="Functions" count={info.methods.length}>
            <Chips items={info.methods} limit={12} />
          </Fold>
        )}
        {info.fields.length > 0 && (
          <Fold title="Fields" count={info.fields.length}>
            <Chips items={info.fields} limit={10} />
          </Fold>
        )}
        {info.calls.length > 0 && (
          <Fold title="Depends on" count={info.calls.length}>
            <Chips items={info.calls} limit={8} onPick={onPick} />
          </Fold>
        )}
        {info.called_by.length > 0 && (
          <Fold title="Used by" count={info.called_by.length}>
            <Chips items={info.called_by} limit={8} onPick={onPick} />
          </Fold>
        )}
      </div>
    </div>
  );
}

function DistrictInspector({
  info,
  onPick,
}: {
  info: DistrictInfo;
  onPick: (n: string) => void;
}) {
  const color = TYPOLOGY_COLORS[info.typology] ?? TYPOLOGY_COLORS.unknown;
  return (
    <div className="flex min-w-0 flex-col">
      <Counter
        icon={<IconCity />}
        value={info.building_count}
        label={info.building_count === 1 ? "building" : "buildings"}
      />
      <Lead
        lead={
          <span className="inline-flex items-center gap-2">
            <span
              className="inline-block size-2.5 rounded-full"
              style={{ background: color }}
            />
            {info.name}.
          </span>
        }
        rest={
          <>
            A {info.typology} district. {info.summary}
          </>
        }
      />

      <Pills>
        <StatPill
          tone="inset"
          icon={<IconCode />}
          value={formatCount(info.total_lines_of_code)}
          label="lines of code"
        />
        <StatPill
          tone="inset"
          icon={<IconFunction />}
          value={formatCount(info.total_methods)}
          label="functions"
        />
      </Pills>

      <div className="mt-6">
        {info.top_buildings.length > 0 && (
          <Fold title="Largest buildings" count={info.top_buildings.length}>
            <Chips items={info.top_buildings} limit={8} onPick={onPick} />
          </Fold>
        )}
        {info.tags.length > 0 && (
          <Fold title="Tags" count={info.tags.length}>
            <Chips items={info.tags} limit={6} />
          </Fold>
        )}
      </div>
    </div>
  );
}
