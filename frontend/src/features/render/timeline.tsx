import { useEffect, useRef, useState } from "react";
import type { CitySummary } from "./types";

/**
 * Scrubs the city back through its own history. A building appears on the day
 * its file did, which comes free from the backend's `git log` pass: nothing is
 * re-parsed and no commit is checked out, so scrubbing is instant. It shows
 * when each part came into existence, not buildings growing.
 */
export function Timeline({
  summary,
  onChange,
}: {
  summary: CitySummary | null;
  onChange: (day: number | null) => void;
}) {
  const [day, setDay] = useState<number | null>(null);
  const [playing, setPlaying] = useState(false);
  const raf = useRef<number | null>(null);

  const total = summary?.history_days ?? 0;

  useEffect(() => {
    setDay(null);
    setPlaying(false);
  }, [summary]);

  useEffect(() => {
    if (!playing || total <= 0) return;
    let last = performance.now();
    let current = day ?? 0;
    const step = (now: number) => {
      // Roughly twelve seconds for the whole history, whatever its length.
      current += ((now - last) / 12000) * total;
      last = now;
      if (current >= total) {
        setDay(null);
        onChange(null);
        setPlaying(false);
        return;
      }
      setDay(Math.round(current));
      onChange(Math.round(current));
      raf.current = requestAnimationFrame(step);
    };
    raf.current = requestAnimationFrame(step);
    return () => {
      if (raf.current !== null) cancelAnimationFrame(raf.current);
    };
    // `day` is the animation's output, not a dependency.
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
          if (playing) {
            setPlaying(false);
          } else {
            setDay(0);
            onChange(0);
            setPlaying(true);
          }
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
          const next = v >= total ? null : v;
          setDay(next);
          onChange(next);
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
