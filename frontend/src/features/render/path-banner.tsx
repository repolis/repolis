import type { PathInfo } from "./types";

/**
 * Shows the state of a path query, which is otherwise invisible: pinning a
 * start building changes nothing on screen until a second building is chosen.
 */
export function PathBanner({
  anchorName,
  path,
  onClear,
}: {
  anchorName: string | null;
  path: PathInfo | null;
  onClear: () => void;
}) {
  if (!anchorName && !path) return null;

  return (
    <div className="absolute left-1/2 top-4 z-40 max-w-[40rem] -translate-x-1/2 rounded border border-gray-700 bg-gray-900/95 px-3 py-2 text-xs">
      {path ? (
        path.hops.length > 0 ? (
          <div className="flex flex-col gap-1">
            <div className="text-gray-400">
              {path.hops.length - 1} hop{path.hops.length === 2 ? "" : "s"} from{" "}
              <span className="text-gray-200">{path.from_name}</span> to{" "}
              <span className="text-gray-200">{path.to_name}</span>
            </div>
            <div className="flex flex-wrap items-center gap-1 font-mono text-[10px] text-gray-300">
              {path.hops.map((h, i) => (
                <span key={i} className="flex items-center gap-1">
                  {i > 0 && <span className="text-gray-600">&rarr;</span>}
                  <span className="rounded bg-gray-800 px-1 py-0.5">{h}</span>
                </span>
              ))}
            </div>
          </div>
        ) : (
          <div className="text-gray-400">
            No dependency path from{" "}
            <span className="text-gray-200">{path.from_name}</span> to{" "}
            <span className="text-gray-200">{path.to_name}</span>.
          </div>
        )
      ) : (
        <div className="text-gray-400">
          Path start: <span className="text-gray-200">{anchorName}</span> &middot;{" "}
          <span className="text-gray-500">
            now select another building and press &ldquo;trace path to here&rdquo;
          </span>
        </div>
      )}
      <button
        onClick={onClear}
        className="absolute right-2 top-1.5 text-gray-500 hover:text-gray-200"
        title="Clear path"
      >
        &times;
      </button>
    </div>
  );
}
