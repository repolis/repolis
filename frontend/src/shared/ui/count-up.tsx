import { useEffect } from "react";
import { animate, motion, useMotionValue, useTransform } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { formatCount } from "@/shared/lib/format";

/**
 * A number that rolls to its value instead of snapping. Rendered through a
 * motion value, so the count never re-renders React on each frame.
 */
export function CountUp({
  value,
  className,
  duration = 1.4,
  delay = 0,
}: {
  value: number;
  className?: string;
  duration?: number;
  delay?: number;
}) {
  const mv = useMotionValue(0);
  const text = useTransform(mv, (v) => formatCount(v));

  useEffect(() => {
    const controls = animate(mv, value, {
      duration,
      delay,
      ease: [0.22, 1, 0.36, 1],
    });
    return () => controls.stop();
  }, [mv, value, duration, delay]);

  return (
    <motion.span className={cn("tabular-nums", className)}>{text}</motion.span>
  );
}
