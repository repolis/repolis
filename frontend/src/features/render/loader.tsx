import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { spring } from "@/shared/lib/motion";
import { Shimmer } from "@/shared/ui/brand";
import { GlassButton, GlassPanel } from "@/shared/ui/glass";
import {
  IconArrowLeft,
  IconDanger,
  IconGithub,
  IconRestart,
} from "@/shared/ui/icons";

export interface StageProgress {
  name: string;
  done: number;
  total: number;
}

/** Pipeline order, mirroring the backend's stage names. */
const STEPS: { id: string; label: string }[] = [
  { id: "engine", label: "Starting the 3D engine" },
  { id: "cloning", label: "Cloning repository" },
  { id: "parsing", label: "Parsing source" },
  { id: "history", label: "Reading git history" },
  { id: "linking", label: "Linking the call graph" },
  { id: "associating", label: "Resolving ambiguous functions" },
  { id: "naming", label: "Naming districts" },
];

function stepIndex(phase: string, stage: StageProgress | null): number {
  if (phase === "booting") return 0;
  if (!stage) return 1;
  const i = STEPS.findIndex((s) => s.id === stage.name);
  return i < 0 ? 1 : i;
}

function useElapsed(running: boolean): string {
  const [start] = useState(() => performance.now());
  const [now, setNow] = useState(start);
  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => setNow(performance.now()), 250);
    return () => window.clearInterval(id);
  }, [running]);
  const s = Math.max(0, Math.floor((now - start) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/**
 * Shown inside the cloud bank while the first city is built. Lists the real
 * pipeline stages as the backend reports them over SSE.
 */
export function AnalysisLoader({
  owner,
  repo,
  phase,
  stage,
  status,
  error,
  onRetry,
  onHome,
}: {
  owner: string;
  repo: string;
  phase: "booting" | "working" | "error";
  stage: StageProgress | null;
  status: string;
  error: string | null;
  onRetry: () => void;
  onHome: () => void;
}) {
  const failed = phase === "error";
  const elapsed = useElapsed(!failed);
  const current = stepIndex(phase, stage);
  const fraction =
    stage && stage.total > 0 ? Math.min(1, stage.done / stage.total) : 0.35;
  const progress = Math.min(0.97, (current + fraction) / STEPS.length);
  // The stage name reads cleaner than the raw status, which repeats the count.
  const headline = stage ? STEPS[current].label : status;

  return (
    <motion.div
      className="absolute inset-0 z-40 flex items-center justify-center px-6"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0, transition: { duration: 0.5 } }}
    >
      <GlassPanel
        strong
        className="w-full max-w-[32rem]"
        innerClassName="p-8"
        initial={{ opacity: 0, y: 24, scale: 0.94, filter: "blur(16px)" }}
        animate={{
          opacity: 1,
          y: 0,
          scale: 1,
          filter: "blur(0px)",
          transitionEnd: { filter: "none" },
        }}
        exit={{
          opacity: 0,
          scale: 1.06,
          filter: "blur(20px)",
          transition: { duration: 0.6, ease: [0.65, 0, 0.35, 1] },
        }}
        transition={{ ...spring.soft, delay: 0.15 }}
      >
        <div className="flex items-center justify-between gap-4">
          <div className="flex min-w-0 items-center gap-2.5 text-[1.5rem] leading-none font-semibold">
            <IconGithub className="size-7 shrink-0 text-white/85" />
            <span className="truncate">
              <span className="text-white/55">{owner}/</span>
              <span className="text-white">{repo}</span>
            </span>
          </div>
          <span className="shrink-0 rounded-full bg-white/10 px-2.5 py-1 text-[0.8125rem] font-semibold text-white/65 tabular-nums">
            {elapsed}
          </span>
        </div>

        <AnimatePresence mode="wait" initial={false}>
          {failed ? (
            <motion.div
              key="error"
              initial={{ opacity: 0, y: 8, filter: "blur(8px)" }}
              animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
              exit={{ opacity: 0, y: -8, filter: "blur(8px)" }}
              transition={spring.glass}
              className="mt-7 flex flex-col gap-6"
            >
              <div className="flex items-start gap-3 rounded-2xl bg-[var(--color-danger-soft)] p-4 text-[1rem] leading-snug font-medium text-white">
                <IconDanger className="mt-0.5 size-5 shrink-0 text-[var(--color-danger)]" />
                <span>{error ?? "Analysis failed"}</span>
              </div>
              <div className="flex gap-2.5">
                <GlassButton
                  tone="primary"
                  size="lg"
                  icon={<IconRestart />}
                  onClick={onRetry}
                >
                  Try again
                </GlassButton>
                <GlassButton
                  size="lg"
                  icon={<IconArrowLeft />}
                  onClick={onHome}
                >
                  Another repository
                </GlassButton>
              </div>
            </motion.div>
          ) : (
            <motion.div
              key="progress"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="mt-7"
            >
              <div className="flex items-baseline justify-between gap-3 text-[1.125rem] font-semibold">
                <Shimmer className="truncate">{headline}</Shimmer>
                {stage && stage.total > 0 && (
                  <span className="shrink-0 text-[0.9375rem] text-white/55 tabular-nums">
                    {stage.done}/{stage.total}
                  </span>
                )}
              </div>

              <div className="relative mt-4 h-1 overflow-hidden rounded-full bg-white/12">
                <motion.div
                  className="absolute inset-y-0 left-0 rounded-full bg-white"
                  initial={{ width: "2%" }}
                  animate={{ width: `${progress * 100}%` }}
                  transition={spring.soft}
                />
                <motion.div
                  className="absolute inset-y-0 w-24 bg-gradient-to-r from-transparent via-white/70 to-transparent"
                  animate={{ x: ["-6rem", "32rem"] }}
                  transition={{
                    duration: 1.6,
                    repeat: Infinity,
                    ease: "easeInOut",
                  }}
                />
              </div>

              <ol className="mt-6 flex flex-col gap-2.5">
                {STEPS.map((s, i) => {
                  const state =
                    i < current ? "done" : i === current ? "active" : "pending";
                  return (
                    <motion.li
                      key={s.id}
                      initial={{ opacity: 0, x: -8 }}
                      animate={{ opacity: 1, x: 0 }}
                      transition={{ ...spring.glass, delay: 0.3 + i * 0.05 }}
                      className={cn(
                        "flex items-center gap-3 text-[0.9375rem] font-semibold transition-colors duration-500",
                        state === "done" && "text-white/60",
                        state === "active" && "text-white",
                        state === "pending" && "text-white/35",
                      )}
                    >
                      <StepMark state={state} />
                      {s.label}
                    </motion.li>
                  );
                })}
              </ol>

              <p className="mt-6 border-t border-white/10 pt-4 text-[0.8125rem] leading-snug font-medium text-white/45">
                The first city is drawn without a model. District names arrive
                afterwards, streamed into the city you are already exploring.
              </p>
            </motion.div>
          )}
        </AnimatePresence>
      </GlassPanel>
    </motion.div>
  );
}

function StepMark({ state }: { state: "done" | "active" | "pending" }) {
  return (
    <span className="relative grid size-[1.125rem] shrink-0 place-items-center">
      <AnimatePresence mode="popLayout" initial={false}>
        {state === "done" ? (
          <motion.svg
            key="done"
            viewBox="0 0 18 18"
            className="size-full"
            initial={{ scale: 0.4, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={spring.snappy}
          >
            <circle cx="9" cy="9" r="9" fill="rgb(255 255 255 / 0.85)" />
            <motion.path
              d="M5.2 9.4l2.4 2.3 5.1-5.3"
              fill="none"
              stroke="#0d0f14"
              strokeWidth="1.8"
              strokeLinecap="round"
              strokeLinejoin="round"
              initial={{ pathLength: 0 }}
              animate={{ pathLength: 1 }}
              transition={{ duration: 0.35, delay: 0.08 }}
            />
          </motion.svg>
        ) : state === "active" ? (
          <motion.span
            key="active"
            className="size-full rounded-full border-2 border-white/20 border-t-white"
            initial={{ scale: 0.6, opacity: 0 }}
            animate={{ scale: 1, opacity: 1, rotate: 360 }}
            transition={{
              rotate: { duration: 0.9, repeat: Infinity, ease: "linear" },
              default: spring.snappy,
            }}
          />
        ) : (
          <motion.span
            key="pending"
            className="size-2 rounded-full bg-white/25"
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
          />
        )}
      </AnimatePresence>
    </span>
  );
}
