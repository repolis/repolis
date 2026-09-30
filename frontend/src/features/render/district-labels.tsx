import { useEffect, useRef, useState } from "react";
import { motion } from "motion/react";

import { district_labels } from "@/wasm/engine";

import { TYPOLOGY_COLORS } from "./types";

interface LabelFrame {
  i: number;
  x: number;
  y: number;
  a: number;
  n: string;
  c: number;
  t: string;
}

type LabelMeta = Pick<LabelFrame, "i" | "n" | "c" | "t">;

/**
 * District names as frosted pins floating over the city. The engine projects
 * each label every frame; this reads the projection and moves the pins
 * directly, so a camera move never re-renders React.
 */
export function DistrictLabels() {
  const [meta, setMeta] = useState<LabelMeta[]>([]);
  const refs = useRef(new Map<number, HTMLDivElement>());
  const signature = useRef("");

  useEffect(() => {
    let raf = 0;
    const tick = () => {
      raf = requestAnimationFrame(tick);
      let frame: LabelFrame[] = [];
      try {
        const raw = district_labels();
        frame = raw ? (JSON.parse(raw) as LabelFrame[]) : [];
      } catch {
        return;
      }

      // The set of districts changes only when a city (re)loads.
      const sig = frame
        .map((l) => `${l.i}:${l.n}:${l.c}`)
        .sort()
        .join("|");
      if (sig !== signature.current && frame.length > 0) {
        signature.current = sig;
        setMeta((prev) => {
          const known = new Map(prev.map((m) => [m.i, m]));
          for (const l of frame)
            known.set(l.i, { i: l.i, n: l.n, c: l.c, t: l.t });
          return [...known.values()];
        });
      }

      const seen = new Set<number>();
      for (const l of frame) {
        const el = refs.current.get(l.i);
        if (!el) continue;
        seen.add(l.i);
        el.style.transform = `translate3d(${l.x.toFixed(1)}px, ${l.y.toFixed(1)}px, 0)`;
        // A custom property, not the wrapper's own opacity: an ancestor
        // with opacity below 1 would switch off the pills' backdrop blur.
        el.style.setProperty(
          "--a",
          String(Math.min(1, Math.max(0.4, l.a * 1.6))),
        );
      }
      for (const [i, el] of refs.current) {
        if (!seen.has(i)) el.style.setProperty("--a", "0");
      }
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  return (
    <div
      aria-hidden
      className="pointer-events-none absolute inset-0 z-[15] overflow-hidden"
    >
      {meta.map((m, k) => (
        <div
          key={`${m.i}:${m.n}`}
          ref={(el) => {
            if (el) refs.current.set(m.i, el);
            else refs.current.delete(m.i);
          }}
          className="absolute top-0 left-0 will-change-transform [--a:0]"
        >
          <motion.div
            initial={{ opacity: 0, y: 10, scale: 0.9 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{
              type: "spring",
              stiffness: 260,
              damping: 26,
              delay: 0.4 + k * 0.06,
            }}
            className="flex -translate-x-1/2 -translate-y-full flex-col items-center"
          >
            <div className="flex h-8 items-center gap-2 rounded-full bg-[rgb(15_20_29/0.66)] pr-3.5 pl-3 text-[0.9375rem] whitespace-nowrap opacity-(--a) transition-opacity duration-300">
              <span
                className="size-2 rounded-full"
                style={{
                  background: TYPOLOGY_COLORS[m.t] ?? TYPOLOGY_COLORS.unknown,
                }}
              />
              <span className="text-white/85">{m.n}</span>
              <span className="text-white/45 tabular-nums">{m.c}</span>
            </div>
            <span className="h-4 w-px bg-gradient-to-b from-white/45 to-white/0 opacity-(--a) transition-opacity duration-300" />
            <span className="size-1.5 rounded-full bg-white/85 opacity-(--a) transition-opacity duration-300" />
          </motion.div>
        </div>
      ))}
    </div>
  );
}
