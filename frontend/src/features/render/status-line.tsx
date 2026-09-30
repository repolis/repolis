import { AnimatePresence, motion } from "motion/react";

import { spring } from "@/shared/lib/motion";
import { GlowRing, Shimmer, StatusDot, type Signal } from "@/shared/ui/brand";
import { IconGithub } from "@/shared/ui/icons";

/**
 * "● Connected to owner/repo", centred under the halo. While the model is
 * still refining, or a regeneration runs, the dot turns amber and the note
 * beside it shimmers.
 */
export function StatusLine({
  owner,
  repo,
  signal,
  note,
  draft = false,
}: {
  owner: string;
  repo: string;
  signal: Signal;
  note: string | null;
  /** The deterministic city, before (or without) the model's refinements. */
  draft?: boolean;
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
      <div className="text-lift absolute top-[4.375rem] left-1/2 z-40 flex -translate-x-1/2 flex-col items-center gap-2">
        <div className="flex items-center gap-3 text-[1.25rem] leading-6 font-semibold whitespace-nowrap">
          <span className="flex items-center gap-2.5">
            <StatusDot signal={signal} />
            <AnimatePresence mode="popLayout" initial={false}>
              <motion.span
                key={lead}
                initial={{ opacity: 0, y: 6, filter: "blur(6px)" }}
                animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
                exit={{ opacity: 0, y: -6, filter: "blur(6px)" }}
                transition={spring.glass}
                className="text-white/85"
              >
                {lead}
              </motion.span>
            </AnimatePresence>
          </span>
          <a
            href={`https://github.com/${owner}/${repo}`}
            target="_blank"
            rel="noreferrer"
            className="group pointer-events-auto flex items-center gap-0.5"
          >
            <IconGithub className="size-6 text-white/85 transition-transform duration-300 group-hover:rotate-[-8deg]" />
            <span className="text-white/65"> {owner}/</span>
            <span className="text-white/85 underline decoration-white/0 underline-offset-4 transition-colors group-hover:decoration-white/50">
              {repo}
            </span>
          </a>
          <AnimatePresence>
            {draft && (
              <motion.span
                initial={{ opacity: 0, scale: 0.8 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.8 }}
                transition={spring.snappy}
                title="Built without a language model: districts are named after their folders"
                className="pointer-events-auto rounded-full bg-white/12 px-2 py-0.5 text-[0.75rem] font-semibold tracking-wide text-white/60 uppercase shadow-[inset_0_1px_0_rgb(255_255_255/0.15)]"
              >
                draft
              </motion.span>
            )}
          </AnimatePresence>
        </div>
        <AnimatePresence>
          {note && (
            <motion.div
              initial={{ opacity: 0, y: -4, filter: "blur(6px)" }}
              animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
              exit={{ opacity: 0, y: -4, filter: "blur(6px)" }}
              transition={spring.glass}
              className="text-[0.9375rem] font-semibold"
            >
              <Shimmer>{note}</Shimmer>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </>
  );
}
