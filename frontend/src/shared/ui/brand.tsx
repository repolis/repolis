import { useId, type ReactNode } from "react";
import LogoMark from "~icons/figma/logo";
import { motion } from "motion/react";

import { cn } from "@/shared/lib/cn";

/** "@ repolis BETA": the Figma text layers exported as outlines, 193x43. */
export function Logo({ className }: { className?: string }) {
  return (
    <LogoMark
      aria-label="repolis beta"
      role="img"
      className={cn("h-[2.6875rem] w-[12.0625rem] shrink-0", className)}
    />
  );
}

/**
 * The Figma overlay layer: each edge darkens to 65% black over its last 18%.
 * It is what keeps white type legible; nothing in the HUD casts a shadow.
 */
export function Vignette({
  className,
  strength = 1,
}: {
  className?: string;
  strength?: number;
}) {
  const edge = "rgba(0,0,0,0.65)";
  return (
    <div
      aria-hidden
      className={cn("pointer-events-none fixed inset-0 z-[25]", className)}
      style={{
        opacity: strength,
        backgroundImage: [
          `linear-gradient(to bottom, ${edge} 0%, rgba(0,0,0,0) 17.9%)`,
          `linear-gradient(to top, ${edge} 0%, rgba(0,0,0,0) 18.2%)`,
          `linear-gradient(to left, ${edge} 0%, rgba(0,0,0,0) 17.8%)`,
          `linear-gradient(to right, ${edge} 0%, rgba(0,0,0,0) 18.3%)`,
        ].join(", "),
      }}
    />
  );
}

export type Signal = "live" | "busy" | "error" | "idle";

const signalColor: Record<Signal, string> = {
  live: "#74FF51",
  busy: "#FFC24D",
  error: "#FF7A70",
  idle: "#FFFFFF",
};

/**
 * The halo above the status line, as exported: a 320px disc blurred by 75
 * inside three rings (25%, 10% and 5% white), half off the top edge.
 */
export function GlowRing({
  signal = "live",
  className,
}: {
  signal?: Signal;
  className?: string;
}) {
  const blurId = `halo${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  return (
    <svg
      aria-hidden
      viewBox="0 0 620 620"
      className={cn(
        "pointer-events-none absolute left-1/2 h-[38.75rem] w-[38.75rem] -translate-x-1/2",
        className,
      )}
      style={{ top: "-31.0625rem" }}
    >
      <defs>
        <filter
          id={blurId}
          x="0"
          y="0"
          width="620"
          height="620"
          filterUnits="userSpaceOnUse"
          colorInterpolationFilters="sRGB"
        >
          <feGaussianBlur stdDeviation="75" />
        </filter>
      </defs>
      <motion.circle
        cx="310"
        cy="310"
        r="160"
        filter={`url(#${blurId})`}
        animate={{
          fill: signalColor[signal],
          opacity: signal === "idle" ? 0.35 : 1,
        }}
        transition={{ duration: 0.8 }}
      />
      <circle
        cx="310"
        cy="310"
        r="198.361"
        fill="none"
        stroke="white"
        strokeOpacity="0.25"
        strokeWidth="2.6988"
      />
      <circle
        cx="310"
        cy="310"
        r="211.855"
        fill="none"
        stroke="white"
        strokeOpacity="0.1"
        strokeWidth="5.39759"
      />
      <circle
        cx="310"
        cy="310"
        r="230.747"
        fill="none"
        stroke="white"
        strokeOpacity="0.05"
        strokeWidth="13.494"
      />
    </svg>
  );
}

/** Figma: a 12px dot with a 4px ring outside it at 25% of the same colour. */
export function StatusDot({
  signal = "live",
  className,
}: {
  signal?: Signal;
  className?: string;
}) {
  const color = signalColor[signal];
  return (
    <svg
      aria-hidden
      viewBox="0 0 20 20"
      className={cn("size-5 shrink-0", className)}
    >
      <motion.circle
        cx="10"
        cy="10"
        r="8"
        fill="none"
        strokeWidth="4"
        initial={false}
        animate={{
          stroke: color,
          strokeOpacity: signal === "idle" ? 0.15 : 0.25,
        }}
        transition={{ duration: 0.5 }}
      />
      <motion.circle
        cx="10"
        cy="10"
        r="6"
        initial={false}
        animate={{ fill: color, opacity: signal === "idle" ? 0.45 : 1 }}
        transition={{ duration: 0.5 }}
      />
    </svg>
  );
}

/** A soft sweep across status text while work is in flight. */
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
      transition={{ repeat: Infinity, duration: 2.2, ease: "linear" }}
      style={{
        backgroundImage:
          "linear-gradient(90deg, transparent 35%, rgb(255 255 255 / 0.95) 50%, transparent 65%), linear-gradient(rgb(255 255 255 / 0.65), rgb(255 255 255 / 0.65))",
      }}
    >
      {children}
    </motion.span>
  );
}
