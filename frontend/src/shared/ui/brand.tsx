import type { ReactNode } from "react";
import { motion } from "motion/react";

import { cn } from "@/shared/lib/cn";

/** "@ repolis BETA", straight from the Figma header. */
export function Logo({
  className,
  compact = false,
}: {
  className?: string;
  compact?: boolean;
}) {
  return (
    <span
      className={cn(
        "text-lift inline-flex items-baseline gap-3 leading-none font-semibold whitespace-nowrap select-none",
        className,
      )}
    >
      <span
        className={cn(
          "tracking-[-0.02em]",
          compact ? "text-[1.75rem]" : "text-[2.25rem]",
        )}
      >
        <span className="text-white/45">@ </span>
        <span className="text-white/65">repolis</span>
      </span>
      <span
        className={cn(
          "text-white/45",
          compact ? "text-[1rem]" : "text-[1.25rem]",
        )}
      >
        BETA
      </span>
    </span>
  );
}

/**
 * Viewfinder brackets 15px in from each corner: the screen reads as a lens
 * onto the city rather than a page.
 */
export function Corners({ className }: { className?: string }) {
  const arm = "absolute rounded-full bg-white/25";
  const corner = (pos: string, flipX: boolean, flipY: boolean) => (
    <span
      className={cn("absolute size-6", pos)}
      style={{ transform: `scale(${flipX ? -1 : 1}, ${flipY ? -1 : 1})` }}
    >
      <span className={cn(arm, "top-0 left-0 h-6 w-1")} />
      <span className={cn(arm, "top-0 left-0 h-1 w-6")} />
    </span>
  );
  return (
    <div
      aria-hidden
      className={cn("pointer-events-none fixed inset-[15px] z-30", className)}
    >
      {corner("top-0 left-0", false, false)}
      {corner("top-0 right-0", true, false)}
      {corner("bottom-0 left-0", false, true)}
      {corner("right-0 bottom-0", true, true)}
    </div>
  );
}

/**
 * The Figma frame's darkening: a soft radial pool plus four edge fades, so
 * white type stays legible over a white city without boxes behind it.
 */
export function Vignette({
  className,
  strength = 0.65,
}: {
  className?: string;
  strength?: number;
}) {
  const edge = "rgba(0,0,0,0.422)";
  return (
    <div
      aria-hidden
      className={cn("pointer-events-none fixed inset-0 z-[25]", className)}
      style={{
        opacity: strength,
        backgroundImage: [
          `radial-gradient(ellipse 72% 72% at 66% 67%, rgba(0,0,0,0) 40%, rgba(0,0,0,0.25) 100%)`,
          `linear-gradient(270deg, rgba(0,0,0,0) 70.6%, ${edge} 100%)`,
          `linear-gradient(90deg, rgba(0,0,0,0) 70.6%, ${edge} 100%)`,
          `linear-gradient(180deg, ${edge} 0%, rgba(0,0,0,0) 29.4%)`,
          `linear-gradient(0deg, ${edge} 0%, rgba(0,0,0,0) 29.4%)`,
        ].join(", "),
      }}
    />
  );
}

export type Signal = "live" | "busy" | "error" | "idle";

const signalColor: Record<Signal, string> = {
  live: "var(--color-signal)",
  busy: "var(--color-amber)",
  error: "var(--color-danger)",
  idle: "rgb(255 255 255 / 0.6)",
};

/**
 * The halo above the status line: a blurred glow inside three thin rings,
 * half off the top of the screen. Its colour is the connection state.
 */
export function GlowRing({
  signal = "live",
  className,
}: {
  signal?: Signal;
  className?: string;
}) {
  const color = signalColor[signal];
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none absolute left-1/2 size-[28rem] -translate-x-1/2",
        className,
      )}
      style={{ top: "-25.6875rem" }}
    >
      <motion.div
        className="absolute inset-[4rem] rounded-full"
        animate={{
          backgroundColor: color,
          opacity: signal === "idle" ? 0.4 : 1,
        }}
        transition={{ duration: 0.8 }}
        style={{ filter: "blur(4.6875rem)" }}
      />
      <div className="animate-breathe absolute inset-0">
        <span className="absolute inset-[1.52rem] rounded-full border-[0.169rem] border-white/25" />
        <span className="absolute inset-[0.59rem] rounded-full border-[0.337rem] border-white/10" />
        <span className="absolute -inset-[0.84rem] rounded-full border-[0.843rem] border-white/5" />
      </div>
    </div>
  );
}

/** 12px dot with a 4px halo, pinging while something is happening. */
export function StatusDot({
  signal = "live",
  className,
}: {
  signal?: Signal;
  className?: string;
}) {
  const color = signalColor[signal];
  return (
    <span className={cn("relative inline-grid size-3 shrink-0", className)}>
      {signal !== "idle" && (
        <span
          className="animate-ping-soft absolute inset-0 rounded-full"
          style={{ background: color }}
        />
      )}
      <motion.span
        className="relative size-3 rounded-full border-4"
        animate={{
          backgroundColor: color,
          borderColor: `color-mix(in srgb, ${color} 25%, transparent)`,
        }}
        transition={{ duration: 0.5 }}
        style={{ backgroundClip: "padding-box" }}
      />
    </span>
  );
}

/** A shimmer that sweeps across status text while work is in flight. */
export function Shimmer({
  children,
  className,
  active = true,
}: {
  children: ReactNode;
  className?: string;
  active?: boolean;
}) {
  if (!active) return <span className={className}>{children}</span>;
  return (
    <motion.span
      className={cn(
        "inline-block [background-size:250%_100%,auto] bg-clip-text [background-repeat:no-repeat,padding-box] text-transparent",
        className,
      )}
      initial={{ backgroundPosition: "100% center" }}
      animate={{ backgroundPosition: "0% center" }}
      transition={{ repeat: Infinity, duration: 1.8, ease: "linear" }}
      style={{
        backgroundImage:
          "linear-gradient(90deg, transparent 35%, white 50%, transparent 65%), linear-gradient(rgb(255 255 255 / 0.62), rgb(255 255 255 / 0.62))",
      }}
    >
      {children}
    </motion.span>
  );
}
