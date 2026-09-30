import { AnimatePresence, motion } from "motion/react";

import { spring } from "@/shared/lib/motion";
import { IconArrowRight, IconCross, IconRouting } from "@/shared/ui/icons";

import type { PathInfo } from "./types";

/** Shows a path query's state: pinning a start changes nothing on screen until
 * a second building is chosen. */
export function PathBanner({
  anchorName,
  path,
  onClear,
}: {
  anchorName: string | null;
  path: PathInfo | null;
  onClear: () => void;
}) {
  const visible = !!(anchorName || path);
  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          key="path"
          initial={{ opacity: 0, y: -12, scale: 0.96 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -8, scale: 0.97 }}
          transition={spring.glass}
          className="glass glass-strong absolute top-[8.25rem] left-1/2 z-40 flex max-w-[min(48rem,calc(100vw-2rem))] -translate-x-1/2 items-start gap-3 rounded-[1.25rem] py-3 pr-12 pl-4"
        >
          <IconRouting className="mt-0.5 size-5 shrink-0 text-[var(--color-signal)]" />
          <div className="flex min-w-0 flex-col gap-2 text-[0.9375rem] font-semibold">
            {path ? (
              path.hops.length > 0 ? (
                <>
                  <div className="text-white/65">
                    <span className="text-white">
                      {path.hops.length - 1} hop
                      {path.hops.length === 2 ? "" : "s"}
                    </span>{" "}
                    from <span className="text-white">{path.from_name}</span> to{" "}
                    <span className="text-white">{path.to_name}</span>
                  </div>
                  <div className="flex flex-wrap items-center gap-1.5">
                    {path.hops.map((h, i) => (
                      <motion.span
                        key={i}
                        initial={{ opacity: 0, x: -6 }}
                        animate={{ opacity: 1, x: 0 }}
                        transition={{ ...spring.glass, delay: i * 0.05 }}
                        className="flex items-center gap-1.5"
                      >
                        {i > 0 && (
                          <IconArrowRight className="size-3.5 text-white/40" />
                        )}
                        <span className="rounded-full bg-white/12 px-2.5 py-1 font-mono text-[0.75rem] text-white/85">
                          {h}
                        </span>
                      </motion.span>
                    ))}
                  </div>
                </>
              ) : (
                <div className="text-white/65">
                  No dependency path from{" "}
                  <span className="text-white">{path.from_name}</span> to{" "}
                  <span className="text-white">{path.to_name}</span>.
                </div>
              )
            ) : (
              <div className="text-white/65">
                Path starts at{" "}
                <span className="font-mono text-white">{anchorName}</span>
                <span className="text-white/45">
                  {" "}
                  · now pick another building and trace the path to it
                </span>
              </div>
            )}
          </div>
          <button
            type="button"
            onClick={onClear}
            aria-label="Clear path"
            title="Clear path"
            className="absolute top-2.5 right-2.5 grid size-7 place-items-center rounded-full text-white/55 hover:bg-white/10 hover:text-white"
          >
            <IconCross className="size-7" />
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
