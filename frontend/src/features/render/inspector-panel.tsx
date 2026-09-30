import { useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { formatAge, formatCount } from "@/shared/lib/format";
import { spring } from "@/shared/lib/motion";
import { Shimmer } from "@/shared/ui/brand";
import { Chip, GlassButton, GlassPanel, StatPill } from "@/shared/ui/glass";
import {
  IconBox,
  IconBranch,
  IconChevronDown,
  IconCity,
  IconCode,
  IconCross,
  IconFile,
  IconFire,
  IconFunction,
  IconGraph,
  IconHashtag,
  IconHistory,
  IconMagic,
  IconRouting,
  IconTarget,
  IconUser,
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

/** Pinned inspector: appears on click, stays until dismissed, so a long field
 * list or a dependency can be read and clicked. Styled after the Figma card:
 * a counter on top, a bold lead sentence, a quieter continuation, and inset
 * pills for the numbers. */
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
        <GlassPanel
          key="inspector"
          strong
          className="absolute top-1/2 right-[8.4375rem] z-40 flex max-h-[calc(100vh-15rem)] w-[27.25rem] -translate-y-1/2 flex-col max-lg:right-6 max-sm:inset-x-3 max-sm:top-auto max-sm:bottom-3 max-sm:w-auto max-sm:translate-y-0"
          innerClassName="flex min-h-0 flex-1 flex-col"
          initial={{ opacity: 0, x: 48, scale: 0.97, filter: "blur(14px)" }}
          animate={{
            opacity: 1,
            x: 0,
            scale: 1,
            filter: "blur(0px)",
            transitionEnd: { filter: "none" },
          }}
          exit={{ opacity: 0, x: 40, scale: 0.98, filter: "blur(10px)" }}
          transition={spring.glass}
        >
          <motion.button
            type="button"
            onClick={onClose}
            title="Close (Esc)"
            aria-label="Close inspector"
            whileHover={{ rotate: 90, scale: 1.08 }}
            whileTap={{ scale: 0.9 }}
            transition={spring.snappy}
            className="absolute top-[0.9375rem] right-[0.9375rem] z-10 grid size-8 place-items-center rounded-full text-white/65 hover:bg-white/10 hover:text-white"
          >
            <IconCross className="size-8" />
          </motion.button>

          <div className="scrollbar-glass min-h-0 flex-1 overflow-x-hidden overflow-y-auto [mask-image:linear-gradient(to_bottom,black_calc(100%-2rem),transparent)] p-8 pb-10">
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={key}
                initial={{ opacity: 0, y: 10, filter: "blur(8px)" }}
                animate={{
                  opacity: 1,
                  y: 0,
                  filter: "blur(0px)",
                  transitionEnd: { filter: "none" },
                }}
                exit={{ opacity: 0, y: -8, filter: "blur(8px)" }}
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
          </div>
        </GlassPanel>
      )}
    </AnimatePresence>
  );
}

/** The Figma header: icon, bright count, quieter noun. */
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
    <div className="flex items-center gap-2 pr-8 text-[1.25rem] leading-6 font-semibold">
      <span className="grid size-[1.375rem] place-items-center text-white [&>svg]:size-full">
        {icon}
      </span>
      <span className="text-white tabular-nums">{formatCount(value)}</span>
      <span className="text-white/65">{label}</span>
    </div>
  );
}

/** First sentence bright, the rest quieter: the Figma body text. */
function LeadText({ lead, rest }: { lead: ReactNode; rest?: ReactNode }) {
  return (
    <p className="mt-4 text-[1.25rem] leading-[1.2] font-semibold break-words text-white">
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

function Section({
  title,
  icon,
  count,
  children,
  defaultOpen = false,
}: {
  title: string;
  icon: ReactNode;
  count?: number;
  children: ReactNode;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div className="border-t border-white/10">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="group flex w-full items-center gap-2.5 py-3 text-left text-[0.9375rem] font-semibold text-white/80 hover:text-white"
      >
        <span className="grid size-[1.125rem] place-items-center text-white/70 [&>svg]:size-full">
          {icon}
        </span>
        <span className="flex-1">{title}</span>
        {count !== undefined && (
          <span className="text-white/45 tabular-nums">{count}</span>
        )}
        <motion.span
          animate={{ rotate: open ? 180 : 0 }}
          transition={spring.snappy}
          className="grid size-4 place-items-center text-white/50 [&>svg]:size-full"
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
          className="rounded-full px-2.5 py-1 text-[0.8125rem] font-semibold text-white/55 hover:text-white"
        >
          {expanded ? "show less" : `+${items.length - limit} more`}
        </button>
      )}
    </div>
  );
}

function Badge({
  children,
  tone,
  title,
}: {
  children: ReactNode;
  tone: "danger" | "amber" | "neutral";
  title?: string;
}) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex items-center rounded-full px-2.5 py-1 text-[0.75rem] font-semibold",
        tone === "danger" &&
          "bg-[var(--color-danger-soft)] text-[var(--color-danger)]",
        tone === "amber" &&
          "bg-[var(--color-amber-soft)] text-[var(--color-amber)]",
        tone === "neutral" && "bg-white/10 text-white/70",
      )}
    >
      {children}
    </span>
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

  const color = TYPOLOGY_COLORS[info.typology] ?? TYPOLOGY_COLORS.unknown;
  const summary = explanation ?? info.summary;
  const [lead, rest] = summary ? splitLead(summary) : ["", ""];

  return (
    <div className="flex min-w-0 flex-col">
      <Counter
        icon={<IconFunction />}
        value={info.num_methods}
        label={info.num_methods === 1 ? "function" : "functions"}
      />

      <div className="mt-4 flex flex-wrap items-center gap-1.5">
        <Chip color={color}>{info.district_name || info.typology}</Chip>
        <Badge tone="neutral">
          {info.kind === "module" ? "File module" : "Type"}
          {info.language ? ` · ${info.language}` : ""}
        </Badge>
        {info.hub && (
          <Badge tone="amber" title="In the top few percent by fan-in">
            hub
          </Badge>
        )}
        {info.cycle_id > 0 && (
          <Badge
            tone="danger"
            title="This building is part of a dependency cycle that crosses module boundaries"
          >
            cycle {info.cycle_id}
          </Badge>
        )}
        {info.assoc_source === "llm" && (
          <Badge
            tone="amber"
            title="Some functions were attributed by the language model, then checked against the AST"
          >
            inferred
          </Badge>
        )}
      </div>

      <LeadText
        lead={<span className="font-mono text-[1.125rem]">{info.name}</span>}
        rest={
          info.source_file ? (
            <span className="font-mono text-[0.875rem] break-all">
              {info.source_file}
            </span>
          ) : undefined
        }
      />

      {summary ? (
        <LeadText lead={lead} rest={rest} />
      ) : (
        <div className="mt-4">
          <GlassButton
            tone="clear"
            size="md"
            icon={<IconMagic />}
            onClick={explain}
            disabled={loading}
            className="glass-inset"
          >
            {loading ? (
              <Shimmer>Reading the source…</Shimmer>
            ) : (
              "Explain this building"
            )}
          </GlassButton>
          {error && (
            <p className="mt-2 text-[0.8125rem] font-semibold text-[var(--color-danger)]">
              {error}
            </p>
          )}
        </div>
      )}

      <div className="mt-6 flex flex-wrap gap-2">
        <StatPill
          tone="inset"
          icon={<IconCode />}
          value={formatCount(info.lines_of_code)}
          label="lines of code"
          className="h-10 px-4 text-[1rem]"
        />
        <StatPill
          tone="inset"
          icon={<IconHashtag />}
          value={formatCount(info.num_fields)}
          label={info.num_fields === 1 ? "field" : "fields"}
          className="h-10 px-4 text-[1rem]"
        />
        <StatPill
          tone="inset"
          icon={<IconGraph />}
          value={info.max_complexity > 0 ? String(info.max_complexity) : "–"}
          label="complexity"
          className="h-10 px-4 text-[1rem]"
        />
        <StatPill
          tone="inset"
          icon={<IconTarget />}
          value={formatCount(info.fan_in)}
          label="used by"
          className="h-10 px-4 text-[1rem]"
        />
        <StatPill
          tone="inset"
          icon={<IconBranch />}
          value={formatCount(info.fan_out)}
          label="uses"
          className="h-10 px-4 text-[1rem]"
        />
      </div>

      <div className="mt-6">
        <Section title="History" icon={<IconHistory />} defaultOpen>
          <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-[0.875rem]">
            <Fact icon={<IconFire />} label="Churn">
              {info.commit_churn} commits
            </Fact>
            <Fact icon={<IconHistory />} label="Last edit">
              {formatAge(info.age_days)}
            </Fact>
            {info.primary_author && (
              <Fact icon={<IconUser />} label="Main author" wide>
                {info.primary_author}
              </Fact>
            )}
            <Fact
              icon={<IconGraph />}
              label="Instability"
              title="Martin's instability: 0 = depended upon, 1 = depends on others"
            >
              {info.instability.toFixed(2)}
            </Fact>
            {info.sum_complexity > 0 && (
              <Fact icon={<IconGraph />} label="Total complexity">
                {info.sum_complexity}
              </Fact>
            )}
          </dl>
        </Section>

        <Section title="Dependency path" icon={<IconRouting />} defaultOpen>
          {anchorId === null ? (
            <GlassButton
              tone="clear"
              size="sm"
              icon={<IconRouting />}
              className="glass-inset"
              onClick={() => onAnchor(info.id)}
            >
              Start a path here
            </GlassButton>
          ) : anchorId === info.id ? (
            <div className="flex items-center gap-3 text-[0.875rem] font-semibold">
              <span className="text-white/65">This is the path start.</span>
              <button
                type="button"
                onClick={() => onAnchor("")}
                className="text-white/45 hover:text-white"
              >
                Cancel
              </button>
            </div>
          ) : (
            <div className="flex flex-wrap items-center gap-2">
              <GlassButton
                tone="primary"
                size="sm"
                icon={<IconRouting />}
                onClick={() => onPath(info.id)}
              >
                Trace path to here
              </GlassButton>
              <button
                type="button"
                onClick={() => onAnchor(info.id)}
                className="px-2 text-[0.8125rem] font-semibold text-white/50 hover:text-white"
              >
                start here instead
              </button>
            </div>
          )}
        </Section>

        {info.methods.length > 0 && (
          <Section
            title="Functions"
            icon={<IconFunction />}
            count={info.methods.length}
          >
            <Chips items={info.methods} limit={12} />
          </Section>
        )}
        {info.fields.length > 0 && (
          <Section
            title="Fields"
            icon={<IconHashtag />}
            count={info.fields.length}
          >
            <Chips items={info.fields} limit={10} />
          </Section>
        )}
        {info.calls.length > 0 && (
          <Section
            title="Depends on"
            icon={<IconBranch />}
            count={info.calls.length}
          >
            <Chips items={info.calls} limit={8} onPick={onPick} />
          </Section>
        )}
        {info.called_by.length > 0 && (
          <Section
            title="Used by"
            icon={<IconTarget />}
            count={info.called_by.length}
          >
            <Chips items={info.called_by} limit={8} onPick={onPick} />
          </Section>
        )}
      </div>
    </div>
  );
}

function Fact({
  icon,
  label,
  children,
  wide,
  title,
}: {
  icon: ReactNode;
  label: string;
  children: ReactNode;
  wide?: boolean;
  title?: string;
}) {
  return (
    <div
      title={title}
      className={cn("flex min-w-0 items-center gap-2", wide && "col-span-2")}
    >
      <span className="grid size-4 shrink-0 place-items-center text-white/45 [&>svg]:size-full">
        {icon}
      </span>
      <dt className="text-white/50">{label}</dt>
      <dd className="truncate font-semibold text-white/90">{children}</dd>
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
  const [lead, rest] = info.summary ? splitLead(info.summary) : ["", ""];
  return (
    <div className="flex min-w-0 flex-col">
      <Counter
        icon={<IconCity />}
        value={info.building_count}
        label={info.building_count === 1 ? "building" : "buildings"}
      />
      <div className="mt-4 flex flex-wrap gap-1.5">
        <Chip color={color}>District · {info.typology}</Chip>
      </div>
      <LeadText lead={info.name} />
      {info.summary && <LeadText lead={lead} rest={rest} />}

      <div className="mt-6 flex flex-wrap gap-2">
        <StatPill
          tone="inset"
          icon={<IconCode />}
          value={formatCount(info.total_lines_of_code)}
          label="lines of code"
          className="h-10 px-4 text-[1rem]"
        />
        <StatPill
          tone="inset"
          icon={<IconFunction />}
          value={formatCount(info.total_methods)}
          label="functions"
          className="h-10 px-4 text-[1rem]"
        />
      </div>

      <div className="mt-6">
        {info.top_buildings.length > 0 && (
          <Section
            title="Largest buildings"
            icon={<IconBox />}
            count={info.top_buildings.length}
            defaultOpen
          >
            <Chips items={info.top_buildings} limit={8} onPick={onPick} />
          </Section>
        )}
        {info.tags.length > 0 && (
          <Section title="Tags" icon={<IconFile />} count={info.tags.length}>
            <Chips items={info.tags} limit={6} />
          </Section>
        )}
      </div>
    </div>
  );
}
