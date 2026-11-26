import { useState } from "react";

interface Props {
  items: string[];
  limit: number;
}

export function Chips({ items, limit }: Props) {
  const [expanded, setExpanded] = useState(false);
  const shown = expanded ? items : items.slice(0, limit);

  return (
    <div className="flex min-w-0 flex-wrap gap-1">
      {shown.map((v, i) => (
        <span
          key={i}
          className="max-w-full rounded bg-gray-800 px-1 py-0.5 text-left font-mono text-[10px] break-all text-gray-300"
        >
          {v}
        </span>
      ))}
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
