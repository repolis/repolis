import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";

import { spring } from "@/shared/lib/motion";
import { Glass } from "@/shared/ui/glass";
import { IconHistory, IconPause, IconPlay } from "@/shared/ui/icons";

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

  // Resetting during render rather than in an effect: a new city must not be
  // scrubbed to the previous one's position, and an effect would let one frame
  // render with the stale day.
  const [shown, setShown] = useState(summary);
  if (shown !== summary) {
    setShown(summary);
    setDay(null);
    setPlaying(false);
  }

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

  const pct = total > 0 ? (at / total) * 100 : 100;

  return (
    <Glass className="hud-dim absolute bottom-10 left-1/2 z-40 flex h-12 w-[26rem] max-w-[calc(100vw-2rem)] -translate-x-1/2 items-center gap-3 pr-[1.125rem] pl-1.5 max-[90rem]:bottom-[7.5rem] max-md:right-20 max-md:bottom-4 max-md:left-4 max-md:w-auto max-md:translate-x-0">
      <motion.button
        type="button"
        onClick={() => {
          if (playing) {
            setPlaying(false);
          } else {
            setDay(0);
            onChange(0);
            setPlaying(true);
          }
        }}
        whileHover={{ scale: 1.06 }}
        whileTap={{ scale: 0.9 }}
        transition={spring.snappy}
        aria-label={playing ? "Stop replay" : "Replay history"}
        title={playing ? "Stop" : "Replay the city's history"}
        className="relative grid size-9 shrink-0 place-items-center rounded-full bg-white text-[#0d0f14]"
      >
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span
            key={playing ? "pause" : "play"}
            initial={{ scale: 0.4, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.4, opacity: 0 }}
            transition={spring.snappy}
            className="grid place-items-center"
          >
            {playing ? (
              <IconPause className="size-4" />
            ) : (
              <IconPlay className="size-4 translate-x-px" />
            )}
          </motion.span>
        </AnimatePresence>
      </motion.button>

      <IconHistory className="relative size-[1.375rem] shrink-0 text-white/65" />

      <input
        type="range"
        min={0}
        max={total}
        value={at}
        aria-label="Day in the repository's history"
        onChange={(e) => {
          setPlaying(false);
          const v = Number(e.target.value);
          const next = v >= total ? null : v;
          setDay(next);
          onChange(next);
        }}
        className="range-glass relative min-w-0 flex-1"
        style={{ ["--fill" as string]: `${pct}%` }}
      />

      <span className="type-hud relative w-[7.5rem] shrink-0 text-right text-white/85 tabular-nums">
        {day === null
          ? "Today"
          : date
            ? date.toISOString().slice(0, 10)
            : `day ${at}`}
      </span>
    </Glass>
  );
}
