import { useLayoutEffect, useState, type ReactNode } from "react";
import { motion } from "motion/react";

import { spring } from "@/shared/lib/motion";

/**
 * Grows and shrinks smoothly with its content instead of snapping, so a
 * panel whose contents change never jumps. Put scrolling on `className`.
 */
export function AutoHeight({
  children,
  className,
  innerClassName,
}: {
  children: ReactNode;
  className?: string;
  innerClassName?: string;
}) {
  const [node, setNode] = useState<HTMLDivElement | null>(null);
  const [height, setHeight] = useState<number | "auto">("auto");

  useLayoutEffect(() => {
    if (!node) return;
    const read = () => setHeight(node.offsetHeight);
    read();
    const ro = new ResizeObserver(read);
    ro.observe(node);
    return () => ro.disconnect();
  }, [node]);

  return (
    <motion.div
      className={className}
      initial={false}
      animate={{ height }}
      transition={spring.glass}
    >
      <div ref={setNode} className={innerClassName}>
        {children}
      </div>
    </motion.div>
  );
}
