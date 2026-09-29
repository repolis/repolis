import { useEffect, useState } from "react";
import type { BuildingInfo, DistrictInfo, SelectPayload } from "./types";
import { TYPOLOGY_COLORS } from "./types";

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

/**
 * Pinned inspector: it appears on click and stays until dismissed, so a long
 * field list or a dependency can actually be read and clicked.
 */
export function InspectorPanel({
  selection,
  repoUrl,
  onClose,
  onPick,
  onAnchor,
  onPath,
  anchorId,
}: Props) {
  if (selection.type === "None") return null;

  return (
    <div className="absolute right-4 top-4 z-50 flex max-h-[calc(100vh-2rem)] w-80 flex-col gap-3 overflow-y-auto overflow-x-hidden rounded border border-gray-700 bg-gray-900/95 p-4 text-xs text-gray-200">
      <button
        onClick={onClose}
        className="absolute right-3 top-3 text-gray-500 hover:text-gray-200"
        title="Close (Esc)"
      >
        &times;
      </button>
      {selection.type === "Building" && (
        <BuildingInspector
          info={selection.data}
          repoUrl={repoUrl}
          onPick={onPick}
          onAnchor={onAnchor}
          onPath={onPath}
          anchorId={anchorId}
        />
      )}
      {selection.type === "District" && <DistrictInspector info={selection.data} onPick={onPick} />}
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
  const chip =
    "max-w-full break-all rounded bg-gray-800 px-1 py-0.5 text-left font-mono text-[10px] text-gray-300";

  return (
    <div className="flex min-w-0 flex-wrap gap-1">
      {shown.map((v, i) =>
        onPick ? (
          <button key={i} onClick={() => onPick(v)} className={`${chip} hover:bg-gray-700 hover:text-white`}>
            {v}
          </button>
        ) : (
          <span key={i} className={chip}>
            {v}
          </span>
        ),
      )}
      {items.length > limit && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="text-[10px] text-gray-500 hover:text-gray-300"
        >
          {expanded ? "show less" : `+${items.length - limit} more`}
        </button>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 border-t border-gray-800 pt-2">
      <div className="font-semibold text-gray-400">{title}</div>
      {children}
    </div>
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

  useEffect(() => {
    setExplanation(null);
    setError(null);
  }, [info.id]);

  // The larger model runs here and only here: one entity, on request, with
  // real source context. Nothing in the render path waits on it.
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

  const age =
    info.age_days > 0
      ? info.age_days < 60
        ? `${info.age_days}d ago`
        : `${Math.round(info.age_days / 30)}mo ago`
      : "unknown";

  return (
    <div className="flex min-w-0 flex-col gap-3">
      <div className="min-w-0 border-b border-gray-700 pb-2 pr-5">
        <div className="flex items-center gap-2 text-[11px] text-gray-400">
          <span
            className="inline-block h-2 w-2 rounded-sm"
            style={{ background: TYPOLOGY_COLORS[info.typology] ?? TYPOLOGY_COLORS.unknown }}
          />
          <span>{info.kind === "module" ? "File module" : "Type"}</span>
          {info.language && <span className="text-gray-500">{info.language}</span>}
          {info.cycle_id > 0 && (
            <span
              className="rounded border border-red-700 px-1 text-red-400"
              title="This building is part of a dependency cycle that crosses module boundaries"
            >
              cycle {info.cycle_id}
            </span>
          )}
          {info.assoc_source === "llm" && (
            <span
              className="rounded border border-amber-700 px-1 text-amber-400"
              title="Some functions were attributed by the language model, then checked against the AST"
            >
              inferred
            </span>
          )}
        </div>
        <div className="mt-1 break-all text-sm font-bold text-white">{info.name}</div>
        {info.source_file && (
          <div className="break-all font-mono text-[11px] text-gray-400">{info.source_file}</div>
        )}
        <div className="mt-1 text-gray-400">
          District: <span className="text-gray-200">{info.district_name}</span>
        </div>
      </div>

      <div className="grid grid-cols-3 gap-2 text-center">
        <Metric label="Lines" value={info.lines_of_code.toLocaleString()} />
        <Metric label="Functions" value={String(info.num_methods)} />
        <Metric label="Fields" value={String(info.num_fields)} />
      </div>

      <div className="grid grid-cols-3 gap-2 text-center">
        <Metric
          label="Complexity"
          value={info.max_complexity > 0 ? `${info.max_complexity}` : "\u2013"}
        />
        <Metric label="Used by" value={String(info.fan_in)} />
        <Metric label="Uses" value={String(info.fan_out)} />
      </div>
      <div className="flex justify-between text-[11px] text-gray-400">
        <span title="Martin's instability: 0 = depended upon, 1 = depends on others">
          Instability: <span className="text-gray-200">{info.instability.toFixed(2)}</span>
        </span>
        {info.hub && <span className="text-amber-400">hub</span>}
        {info.sum_complexity > 0 && <span>total complexity {info.sum_complexity}</span>}
      </div>

      <div className="flex justify-between text-[11px] text-gray-400">
        <span>
          Churn: <span className="text-gray-200">{info.commit_churn}</span> commits
        </span>
        <span>
          Last edit: <span className="text-gray-200">{age}</span>
        </span>
      </div>
      {info.primary_author && (
        <div className="text-[11px] text-gray-400">
          Main author: <span className="text-gray-200">{info.primary_author}</span>
        </div>
      )}

      <div className="flex flex-col gap-1">
        {explanation ? (
          <div className="rounded bg-gray-800 p-2 leading-relaxed text-gray-300">{explanation}</div>
        ) : (
          <button
            onClick={explain}
            disabled={loading}
            className="rounded border border-gray-700 px-2 py-1.5 text-gray-300 hover:border-gray-500 disabled:opacity-50"
          >
            {loading ? "Asking the model…" : "Explain this"}
          </button>
        )}
        {error && <div className="text-[11px] text-red-400">{error}</div>}
      </div>

      <Section title="Dependency path">
        {anchorId === null ? (
          <button
            onClick={() => onAnchor(info.id)}
            className="self-start rounded border border-gray-700 px-2 py-1 text-gray-300 hover:border-gray-500"
          >
            Start a path here
          </button>
        ) : anchorId === info.id ? (
          <div className="flex items-center gap-2">
            <span className="text-gray-400">This is the path start.</span>
            <button
              onClick={() => onAnchor("")}
              className="text-gray-500 hover:text-gray-300"
            >
              cancel
            </button>
          </div>
        ) : (
          <div className="flex items-center gap-2">
            <button
              onClick={() => onPath(info.id)}
              className="rounded border border-gray-500 bg-gray-800 px-2 py-1 text-white hover:border-gray-400"
            >
              Trace path to here
            </button>
            <button
              onClick={() => onAnchor(info.id)}
              className="text-gray-500 hover:text-gray-300"
            >
              start here instead
            </button>
          </div>
        )}
      </Section>

      {info.methods.length > 0 && (
        <Section title={`Functions (${info.methods.length})`}>
          <Chips items={info.methods} limit={12} />
        </Section>
      )}
      {info.fields.length > 0 && (
        <Section title={`Fields (${info.fields.length})`}>
          <Chips items={info.fields} limit={10} />
        </Section>
      )}
      {info.calls.length > 0 && (
        <Section title={`Depends on (${info.calls.length})`}>
          <Chips items={info.calls} limit={8} onPick={onPick} />
        </Section>
      )}
      {info.called_by.length > 0 && (
        <Section title={`Used by (${info.called_by.length})`}>
          <Chips items={info.called_by} limit={8} onPick={onPick} />
        </Section>
      )}
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded border border-gray-700 p-1.5">
      <div className="text-[10px] text-gray-400">{label}</div>
      <div className="truncate font-semibold text-white">{value}</div>
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
  return (
    <div className="flex min-w-0 flex-col gap-3">
      <div className="min-w-0 border-b border-gray-700 pb-2 pr-5">
        <div className="flex items-center gap-2 text-[11px] text-gray-400">
          <span
            className="inline-block h-2 w-2 rounded-sm"
            style={{ background: TYPOLOGY_COLORS[info.typology] ?? TYPOLOGY_COLORS.unknown }}
          />
          <span>District &middot; {info.typology}</span>
        </div>
        <div className="mt-1 break-all text-sm font-bold text-white">{info.name}</div>
        {info.summary && <div className="mt-1 text-gray-300">{info.summary}</div>}
      </div>

      <div className="grid grid-cols-3 gap-2 text-center">
        <Metric label="Entities" value={String(info.building_count)} />
        <Metric label="Functions" value={String(info.total_methods)} />
        <Metric label="Lines" value={info.total_lines_of_code.toLocaleString()} />
      </div>

      {info.tags.length > 0 && (
        <Section title="Tags">
          <Chips items={info.tags} limit={6} />
        </Section>
      )}
      {info.top_buildings.length > 0 && (
        <Section title="Largest entities">
          <Chips items={info.top_buildings} limit={8} onPick={onPick} />
        </Section>
      )}
    </div>
  );
}
