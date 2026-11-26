import { useState } from "react";

import type { BuildingInfo } from "../../../types";
import { TYPOLOGY_COLORS } from "../../../utils/colors";
import { Chips } from "./chips";
import { FileGroups } from "./file-groups";
import { Metric } from "./metric";
import { Section } from "./section";

function formatAge(days: number): string {
  if (days <= 0) return "unknown";
  if (days < 60) return `${days}d ago`;
  return `${Math.round(days / 30)}mo ago`;
}

interface Props {
  info: BuildingInfo;
  repoUrl: string;
  commit: string | null;
  onPick: (n: string) => void;
  onAnchor: (id: string) => void;
  onPath: (id: string) => void;
  anchorId: string | null;
}

export function BuildingInspector({
  info,
  repoUrl,
  commit,
  onPick,
  onAnchor,
  onPath,
  anchorId,
}: Props) {
  const [explanation, setExplanation] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // The larger model runs only here; nothing in the render path waits on it.
  async function explain() {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch("/api/explain", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          repo_url: repoUrl,
          commit,
          building_id: info.id,
        }),
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

  const age = formatAge(info.age_days);

  return (
    <div className="flex min-w-0 flex-col gap-3">
      <div className="min-w-0 border-b border-gray-700 pr-5 pb-2">
        <div className="flex items-center gap-2 text-[11px] text-gray-400">
          <span
            className="inline-block h-2 w-2 rounded-sm"
            style={{
              background:
                TYPOLOGY_COLORS[info.typology] ?? TYPOLOGY_COLORS.unknown,
            }}
          />
          <span>{info.kind === "module" ? "File module" : "Type"}</span>
          {info.language && (
            <span className="text-gray-500">{info.language}</span>
          )}
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
        <div className="mt-1 text-sm font-bold break-all text-white">
          {info.name}
        </div>
        {info.source_file && (
          <div className="font-mono text-[11px] break-all text-gray-400">
            {info.source_file}
          </div>
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
          Instability:{" "}
          <span className="text-gray-200">{info.instability.toFixed(2)}</span>
        </span>
        {info.hub && <span className="text-amber-400">hub</span>}
        {info.sum_complexity > 0 && (
          <span>total complexity {info.sum_complexity}</span>
        )}
      </div>

      <div className="flex justify-between text-[11px] text-gray-400">
        <span>
          Churn: <span className="text-gray-200">{info.commit_churn}</span>{" "}
          commits
        </span>
        <span>
          Last edit: <span className="text-gray-200">{age}</span>
        </span>
      </div>
      {info.primary_author && (
        <div className="text-[11px] text-gray-400">
          Main author:{" "}
          <span className="text-gray-200">{info.primary_author}</span>
        </div>
      )}

      <div className="flex flex-col gap-1">
        {explanation ? (
          <div className="rounded bg-gray-800 p-2 leading-relaxed text-gray-300">
            {explanation}
          </div>
        ) : (
          <button
            onClick={explain}
            disabled={loading}
            className="rounded border border-gray-700 px-2 py-1.5 text-gray-300 hover:border-gray-500 disabled:opacity-50"
          >
            {loading ? "Asking the model..." : "Explain this"}
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
          <FileGroups refs={info.calls} onPick={onPick} />
        </Section>
      )}
      {info.called_by.length > 0 && (
        <Section title={`Used by (${info.called_by.length})`}>
          <FileGroups refs={info.called_by} onPick={onPick} />
        </Section>
      )}
    </div>
  );
}
