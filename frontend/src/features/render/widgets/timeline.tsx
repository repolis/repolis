import { useEffect, useState } from "react";

import type { CitySummary } from "../types";

interface Props {
  summary: CitySummary | null;
  day: number | null;
  onChange: (day: number | null) => void;
}

export function Timeline({ summary, day, onChange }: Props) {
  const [playing, setPlaying] = useState(false);
  const total = summary?.history_days ?? 0;

  // About twelve seconds from first commit to today, whatever the history length.
  useEffect(() => {
    if (!playing || total <= 0) return;
    let frame = 0;
    let last = performance.now();
    let current = 0;
    const step = (now: number) => {
      current += ((now - last) / 12000) * total;
      last = now;
      if (current >= total) {
        onChange(null);
        setPlaying(false);
        return;
      }
      onChange(Math.round(current));
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [playing, total, onChange]);

  if (!summary || total <= 0) return null;

  const at = day ?? total;
  const date = summary.first_commit
    ? new Date(new Date(summary.first_commit).getTime() + at * 86400000)
    : null;

  return (
    <div className="absolute bottom-4 left-1/2 z-40 flex w-[34rem] -translate-x-1/2 items-center gap-3 rounded border border-gray-700 bg-gray-900/95 px-3 py-2 text-xs">
      <button
        onClick={() => {
          if (!playing) onChange(0);
          setPlaying(!playing);
        }}
        className="w-12 shrink-0 rounded border border-gray-700 px-1.5 py-0.5 text-gray-300 hover:border-gray-500"
      >
        {playing ? "Stop" : "Play"}
      </button>

      <input
        type="range"
        min={0}
        max={total}
        value={at}
        onChange={(e) => {
          setPlaying(false);
          const v = Number(e.target.value);
          onChange(v >= total ? null : v);
        }}
        className="flex-1"
      />

      <span className="w-28 shrink-0 text-right text-gray-400">
        {day === null
          ? "today"
          : date
            ? date.toISOString().slice(0, 10)
            : `day ${at}`}
      </span>
    </div>
  );
}
