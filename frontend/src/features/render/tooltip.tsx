import type { HoverInfo } from "./types";

/** Cursor tooltip, deliberately tiny: hover detail vanishes the moment the
 * pointer moves toward it, so the full record lives in the pinned inspector. */
export function Tooltip({
  hover,
  pos,
}: {
  hover: HoverInfo | null;
  pos: { x: number; y: number };
}) {
  if (!hover) return null;
  return (
    <div
      className="pointer-events-none absolute z-40 rounded border border-gray-700 bg-gray-900/95 px-2 py-1 text-xs text-gray-200"
      style={{ left: pos.x + 14, top: pos.y + 14 }}
    >
      <div className="font-semibold text-white">{hover.name}</div>
      <div className="text-[10px] text-gray-400">{hover.detail}</div>
    </div>
  );
}
