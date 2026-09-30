import { AnimatePresence, motion } from "motion/react";

import { spring } from "@/shared/lib/motion";
import { GlowRing, StatusDot, type Signal } from "@/shared/ui/brand";
import { IconGithub } from "@/shared/ui/icons";

/**
 * "● Connected to owner/repo" under the halo, laid out as in Figma: 10px
 * from dot to text, 12px between the two groups, 2px from icon to name.
 * While work streams in the dot turns amber and a quiet note appears below.
 */
export function StatusLine({
  owner,
  repo,
  signal,
  note,
}: {
  owner: string;
  repo: string;
  signal: Signal;
  note: string | null;
}) {
  const lead =
    signal === "error"
      ? "Lost connection to"
      : signal === "busy" && !note
        ? "Connecting to"
        : "Connected to";

  return (
    <>
      <GlowRing signal={signal} className="z-0" />
      <div className="type-hud absolute top-[3.25rem] left-1/2 z-40 flex -translate-x-1/2 flex-col items-center gap-1.5 max-md:top-[5.5rem]">
        {/* The parts glide when the lead changes width; the row itself is
            not animated, which would stretch the type. */}
        <div className="flex items-center gap-3 whitespace-nowrap">
          <motion.span
            layout="position"
            transition={spring.glass}
            className="flex items-center gap-2.5"
          >
            <StatusDot signal={signal} className="-m-1" />
            <AnimatePresence mode="popLayout" initial={false}>
              <motion.span
                key={lead}
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={spring.glass}
                className="text-white/85"
              >
                {lead}
              </motion.span>
            </AnimatePresence>
          </motion.span>
          <motion.a
            layout="position"
            transition={spring.glass}
            href={`https://github.com/${owner}/${repo}`}
            target="_blank"
            rel="noreferrer"
            className="group pointer-events-auto flex items-center gap-0.5"
          >
            <IconGithub className="size-6 transition-transform duration-300 ease-[var(--ease-glass)] group-hover:scale-110" />
            <span className="text-white/65 transition-colors duration-300 group-hover:text-white/85">
              {" "}
              {owner}/
            </span>
            <span className="text-white/85 transition-colors duration-300 group-hover:text-white">
              {repo}
            </span>
          </motion.a>
        </div>
        <AnimatePresence>
          {note && (
            <motion.div
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={spring.glass}
              className="text-[1.0625rem] text-white/65"
            >
              {note}
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </>
  );
}
