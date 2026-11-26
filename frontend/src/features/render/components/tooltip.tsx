import { useEffect, useState } from "react";

import type { HoverInfo } from "../types";

// Tracks the cursor itself so mouse moves re-render only this.
interface Props {
  hover: HoverInfo | null;
}

export function Tooltip({ hover }: Props) {
  const [pos, setPos] = useState({ x: 0, y: 0 });

  useEffect(() => {
    const onMove = (e: MouseEvent) => setPos({ x: e.clientX, y: e.clientY });
    window.addEventListener("mousemove", onMove);
    return () => window.removeEventListener("mousemove", onMove);
  }, []);

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
