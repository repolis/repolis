import { useEffect } from "react";
import { AnimatePresence, motion, useSpring } from "motion/react";

import { spring } from "@/shared/lib/motion";

import type { HoverInfo } from "./types";

/** Cursor tooltip, deliberately tiny: hover detail vanishes the moment the
 * pointer moves toward it, so the full record lives in the pinned inspector.
 * It tracks the pointer itself, on a spring, so hovering never re-renders
 * the rest of the interface. */
export function Tooltip({ hover }: { hover: HoverInfo | null }) {
  const x = useSpring(0, spring.follow);
  const y = useSpring(0, spring.follow);

  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      x.set(e.clientX + 18);
      y.set(e.clientY + 18);
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    return () => window.removeEventListener("pointermove", onMove);
  }, [x, y]);

  return (
    <motion.div
      className="pointer-events-none fixed top-0 left-0 z-50"
      style={{ x, y }}
    >
      <AnimatePresence>
        {hover && (
          <motion.div
            key="tip"
            initial={{ opacity: 0, scale: 0.9, y: 4 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.94 }}
            transition={spring.snappy}
            style={{ transformOrigin: "top left" }}
            className="flex max-w-[24rem] flex-col gap-0.5 rounded-2xl bg-[rgb(15_20_29/0.55)] px-4 py-2.5 backdrop-blur-xl"
          >
            <span className="truncate text-[1.0625rem] text-white">
              {hover.name}
            </span>
            <span className="truncate text-[0.9375rem] text-white/65">
              {hover.detail}
            </span>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
}
