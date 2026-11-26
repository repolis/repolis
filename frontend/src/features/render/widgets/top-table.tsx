import { useState } from "react";

import type { BuildingInfo } from "../types";

interface Column {
  label: string;
  value: (b: BuildingInfo) => number;
}

const COLUMNS: Column[] = [
  { label: "Functions", value: (b) => b.num_methods },
  { label: "Complexity", value: (b) => b.max_complexity },
  { label: "Used by", value: (b) => b.fan_in },
  { label: "Depends on", value: (b) => b.fan_out },
  { label: "Churn", value: (b) => b.commit_churn },
];

const PAGE_SIZE = 25;

interface Props {
  buildings: BuildingInfo[];
  onPick: (id: string) => void;
}

export function TopTable({ buildings, onPick }: Props) {
  const [sortColumn, setSortColumn] = useState(0);
  const [ascending, setAscending] = useState(false);
  const [shown, setShown] = useState(PAGE_SIZE);

  const value = COLUMNS[sortColumn].value;
  const rows = [...buildings]
    .sort((a, b) => (ascending ? value(a) - value(b) : value(b) - value(a)))
    .slice(0, shown);

  function sortBy(column: number) {
    if (column === sortColumn) {
      setAscending(!ascending);
    } else {
      setSortColumn(column);
      setAscending(false);
    }
  }

  return (
    <div className="mt-1 max-h-[50vh] overflow-y-auto rounded border border-gray-700 bg-gray-900/95">
      <table className="w-full text-left">
        <thead className="sticky top-0 bg-gray-900 text-[10px] text-gray-500">
          <tr>
            <th className="px-2 py-1 font-normal">Name</th>
            {COLUMNS.map((column, i) => (
              <th
                key={column.label}
                className="px-1 py-1 text-right font-normal"
              >
                <button
                  onClick={() => sortBy(i)}
                  className={
                    i === sortColumn ? "text-gray-200" : "hover:text-gray-300"
                  }
                >
                  {column.label}
                  {i === sortColumn && (ascending ? " ^" : " v")}
                </button>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((b) => (
            <tr
              key={b.id}
              onClick={() => onPick(b.id)}
              className="cursor-pointer text-gray-300 hover:bg-gray-800"
            >
              <td
                className="max-w-36 truncate px-2 py-0.5 font-mono"
                title={`${b.name} (${b.district_name})`}
              >
                {b.name}
              </td>
              {COLUMNS.map((column) => (
                <td key={column.label} className="px-1 text-right tabular-nums">
                  {column.value(b)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {buildings.length > shown && (
        <button
          onClick={() => setShown(shown + PAGE_SIZE)}
          className="w-full py-1 text-gray-500 hover:text-gray-300"
        >
          show more ({buildings.length - shown})
        </button>
      )}
    </div>
  );
}
