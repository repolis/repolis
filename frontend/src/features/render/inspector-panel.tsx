export interface BuildingHoverInfo {
  name: string;
  source_file: string;
  district_name: string;
  typology: string;
  num_fields: number;
  num_methods: number;
  lines_of_code: number;
  commit_churn: number;
  last_modified: string;
  summary: string;
  fields: string[];
  methods: string[];
  dependencies: string[];
  callers: string[];
}

export interface DistrictHoverInfo {
  name: string;
  typology: string;
  summary: string;
  tags: string[];
  building_count: number;
  total_lines_of_code: number;
  buildings: string[];
}

export interface RoadHoverInfo {
  name: string;
  road_type: string;
  source: string;
  target: string;
  weight: number;
}

export type HoverData =
  | { type: "Building"; data: BuildingHoverInfo }
  | { type: "District"; data: DistrictHoverInfo }
  | { type: "Road"; data: RoadHoverInfo }
  | { type: "None" };

interface InspectorPanelProps {
  hover: HoverData | null;
}

export function InspectorPanel({ hover }: InspectorPanelProps) {
  if (!hover || hover.type === "None") {
    return null;
  }

  return (
    <div className="pointer-events-none absolute right-4 top-4 z-50 flex max-h-[calc(100vh-2rem)] w-80 flex-col gap-3 overflow-y-auto rounded border border-gray-700 bg-gray-900 p-4 text-xs text-gray-200">
      {hover.type === "Building" && <BuildingInspector info={hover.data} />}
      {hover.type === "District" && <DistrictInspector info={hover.data} />}
      {hover.type === "Road" && <RoadInspector info={hover.data} />}
    </div>
  );
}

function BuildingInspector({ info }: { info: BuildingHoverInfo }) {
  return (
    <div className="flex flex-col gap-3">
      {/* Header */}
      <div className="border-b border-gray-700 pb-2">
        <div className="flex items-center justify-between text-[11px] text-gray-400">
          <span>Building</span>
          <span className="rounded border border-gray-700 px-1.5 py-0.5 text-gray-300">
            {info.typology || "Module"}
          </span>
        </div>
        <div className="mt-1 text-sm font-bold text-white">{info.name}</div>
        {info.source_file && (
          <div className="break-all font-mono text-[11px] text-gray-400">
            {info.source_file}
          </div>
        )}
        <div className="mt-1 text-gray-400">
          District: <span className="text-gray-200">{info.district_name}</span>
        </div>
      </div>

      {/* Summary */}
      {info.summary && (
        <div className="rounded bg-gray-800 p-2 leading-relaxed text-gray-300">
          {info.summary}
        </div>
      )}

      {/* Metrics */}
      <div className="grid grid-cols-3 gap-2 text-center">
        <div className="rounded border border-gray-700 p-1.5">
          <div className="text-[10px] text-gray-400">Lines</div>
          <div className="font-semibold text-white">
            {info.lines_of_code.toLocaleString()}
          </div>
        </div>
        <div className="rounded border border-gray-700 p-1.5">
          <div className="text-[10px] text-gray-400">Methods</div>
          <div className="font-semibold text-white">{info.num_methods}</div>
        </div>
        <div className="rounded border border-gray-700 p-1.5">
          <div className="text-[10px] text-gray-400">Fields</div>
          <div className="font-semibold text-white">{info.num_fields}</div>
        </div>
      </div>

      {/* Churn & Modified */}
      {(info.commit_churn > 0 || info.last_modified) && (
        <div className="flex justify-between border-t border-gray-800 pt-1 text-[11px] text-gray-400">
          {info.commit_churn > 0 && (
            <span>Churn: {info.commit_churn} commits</span>
          )}
          {info.last_modified && <span>Modified: {info.last_modified}</span>}
        </div>
      )}

      {/* Methods */}
      {info.methods && info.methods.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className="font-semibold text-gray-400">
            Methods ({info.methods.length})
          </div>
          <div className="flex max-h-24 flex-wrap gap-1 overflow-y-auto">
            {info.methods.slice(0, 15).map((m, i) => (
              <span
                key={i}
                className="rounded bg-gray-800 px-1 py-0.5 font-mono text-[10px] text-gray-300"
              >
                {m}
              </span>
            ))}
            {info.methods.length > 15 && (
              <span className="text-gray-500">
                +{info.methods.length - 15} more
              </span>
            )}
          </div>
        </div>
      )}

      {/* Fields */}
      {info.fields && info.fields.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className="font-semibold text-gray-400">
            Fields ({info.fields.length})
          </div>
          <div className="flex max-h-20 flex-wrap gap-1 overflow-y-auto">
            {info.fields.slice(0, 10).map((f, i) => (
              <span
                key={i}
                className="rounded bg-gray-800 px-1 py-0.5 font-mono text-[10px] text-gray-300"
              >
                {f}
              </span>
            ))}
            {info.fields.length > 10 && (
              <span className="text-gray-500">
                +{info.fields.length - 10} more
              </span>
            )}
          </div>
        </div>
      )}

      {/* Dependencies */}
      {info.dependencies && info.dependencies.length > 0 && (
        <div className="flex flex-col gap-1 border-t border-gray-800 pt-2">
          <div className="font-semibold text-gray-400">
            Depends On ({info.dependencies.length})
          </div>
          <div className="flex max-h-20 flex-wrap gap-1 overflow-y-auto">
            {info.dependencies.slice(0, 8).map((dep, i) => (
              <span
                key={i}
                className="rounded bg-gray-800 px-1 py-0.5 font-mono text-[10px] text-gray-300"
              >
                {dep}
              </span>
            ))}
            {info.dependencies.length > 8 && (
              <span className="text-gray-500">
                +{info.dependencies.length - 8} more
              </span>
            )}
          </div>
        </div>
      )}

      {/* Callers */}
      {info.callers && info.callers.length > 0 && (
        <div className="flex flex-col gap-1 border-t border-gray-800 pt-2">
          <div className="font-semibold text-gray-400">
            Referenced By ({info.callers.length})
          </div>
          <div className="flex max-h-20 flex-wrap gap-1 overflow-y-auto">
            {info.callers.slice(0, 8).map((c, i) => (
              <span
                key={i}
                className="rounded bg-gray-800 px-1 py-0.5 font-mono text-[10px] text-gray-300"
              >
                {c}
              </span>
            ))}
            {info.callers.length > 8 && (
              <span className="text-gray-500">
                +{info.callers.length - 8} more
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function DistrictInspector({ info }: { info: DistrictHoverInfo }) {
  return (
    <div className="flex flex-col gap-3">
      {/* Header */}
      <div className="border-b border-gray-700 pb-2">
        <div className="flex items-center justify-between text-[11px] text-gray-400">
          <span>District</span>
          <span className="rounded border border-gray-700 px-1.5 py-0.5 text-gray-300">
            {info.typology}
          </span>
        </div>
        <div className="mt-1 text-sm font-bold text-white">{info.name}</div>
      </div>

      {/* Summary */}
      {info.summary && (
        <div className="rounded bg-gray-800 p-2 leading-relaxed text-gray-300">
          {info.summary}
        </div>
      )}

      {/* Metrics */}
      <div className="grid grid-cols-2 gap-2 text-center">
        <div className="rounded border border-gray-700 p-1.5">
          <div className="text-[10px] text-gray-400">Buildings</div>
          <div className="font-semibold text-white">{info.building_count}</div>
        </div>
        <div className="rounded border border-gray-700 p-1.5">
          <div className="text-[10px] text-gray-400">Total LOC</div>
          <div className="font-semibold text-white">
            {info.total_lines_of_code.toLocaleString()}
          </div>
        </div>
      </div>

      {/* Tags */}
      {info.tags && info.tags.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className="font-semibold text-gray-400">Tags</div>
          <div className="flex flex-wrap gap-1">
            {info.tags.map((tag, i) => (
              <span
                key={i}
                className="rounded border border-gray-700 px-1.5 py-0.5 text-[10px] text-gray-300"
              >
                #{tag}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Buildings */}
      {info.buildings && info.buildings.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className="font-semibold text-gray-400">
            Modules ({info.buildings.length})
          </div>
          <div className="flex max-h-36 flex-wrap gap-1 overflow-y-auto">
            {info.buildings.map((b, i) => (
              <span
                key={i}
                className="rounded bg-gray-800 px-1 py-0.5 font-mono text-[10px] text-gray-300"
              >
                {b}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function RoadInspector({ info }: { info: RoadHoverInfo }) {
  return (
    <div className="flex flex-col gap-3">
      {/* Header */}
      <div className="border-b border-gray-700 pb-2">
        <div className="flex items-center justify-between text-[11px] text-gray-400">
          <span>Road</span>
          <span className="rounded border border-gray-700 px-1.5 py-0.5 text-gray-300">
            {info.road_type}
          </span>
        </div>
        <div className="mt-1 text-sm font-bold text-white">{info.name}</div>
      </div>

      {/* Connection info */}
      {info.source && info.target && (
        <div className="space-y-1 rounded border border-gray-700 p-2">
          <div>
            <span className="text-gray-400">Source:</span>{" "}
            <span className="font-mono text-gray-200">{info.source}</span>
          </div>
          <div>
            <span className="text-gray-400">Target:</span>{" "}
            <span className="font-mono text-gray-200">{info.target}</span>
          </div>
          <div>
            <span className="text-gray-400">Weight:</span>{" "}
            <span className="font-semibold text-white">{info.weight}</span>
          </div>
        </div>
      )}

      <div className="text-[11px] text-gray-400">
        Road width reflects dependency weight and communication density.
      </div>
    </div>
  );
}
