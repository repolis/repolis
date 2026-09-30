import { AnimatePresence, motion } from "motion/react";

import { GLASS } from "@/shared/glass/maps";
import { spring } from "@/shared/lib/motion";
import { Glass, SHEET_FILL } from "@/shared/ui/glass";
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
        <Glass
          key="path"
          params={GLASS.sheet}
          radius={24}
          fill={SHEET_FILL}
          initial={{ opacity: 0, y: -14 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -8 }}
          transition={spring.glass}
          className="hud-dim absolute top-[7.5rem] left-1/2 z-40 flex max-w-[min(48rem,calc(100vw-2rem))] -translate-x-1/2 items-start gap-3 py-4 pr-14 pl-5"
        >
          <IconRouting className="relative mt-0.5 size-[1.375rem] shrink-0 text-[var(--color-signal)]" />
          <div className="relative flex min-w-0 flex-col gap-2.5 text-[1.0625rem]">
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
                          <IconArrowRight className="size-3.5 text-white/45" />
                        )}
                        <span className="rounded-full bg-white/10 px-3 py-1 text-[0.9375rem] text-white/85">
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
                Path starts at <span className="text-white">{anchorName}</span>
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
            className="absolute top-3 right-3 grid size-8 place-items-center rounded-full opacity-100 transition-opacity duration-200 hover:opacity-75"
          >
            <IconCross className="size-8" />
          </button>
        </Glass>
      )}
    </AnimatePresence>
  );
}
