import type { SelectPayload } from "../../types";
import { BuildingInspector } from "./components/building-inspector";
import { DistrictInspector } from "./components/district-inspector";

interface Props {
  selection: SelectPayload;
  repoUrl: string;
  onClose: () => void;
  onPick: (nameOrId: string) => void;
  onAnchor: (buildingId: string) => void;
  onPath: (buildingId: string) => void;
  anchorId: string | null;
}

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
    <div className="absolute top-4 right-4 z-50 flex max-h-[calc(100vh-2rem)] w-80 flex-col gap-3 overflow-x-hidden overflow-y-auto rounded border border-gray-700 bg-gray-900/95 p-4 text-xs text-gray-200">
      <button
        onClick={onClose}
        className="absolute top-3 right-3 text-gray-500 hover:text-gray-200"
        title="Close (Esc)"
      >
        x
      </button>
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
    </div>
  );
}
