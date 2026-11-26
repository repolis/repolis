import { useState } from "react";

interface Props {
  items: string[];
  limit: number;
  onPick?: (n: string) => void;
}

export function Chips({ items, limit, onPick }: Props) {
  const [expanded, setExpanded] = useState(false);
  const shown = expanded ? items : items.slice(0, limit);
  const chip =
    "max-w-full break-all rounded bg-gray-800 px-1 py-0.5 text-left font-mono text-[10px] text-gray-300";

  return (
    <div className="flex min-w-0 flex-wrap gap-1">
      {shown.map((v, i) =>
        onPick ? (
          <button
            key={i}
            onClick={() => onPick(v)}
            className={`${chip} hover:bg-gray-700 hover:text-white`}
          >
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
