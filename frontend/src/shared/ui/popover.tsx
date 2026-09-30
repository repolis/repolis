import { useEffect, useRef, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";

import { GLASS } from "@/shared/glass/maps";
import { cn } from "@/shared/lib/cn";
import { ease, spring } from "@/shared/lib/motion";
import { Glass, SHEET_FILL } from "@/shared/ui/glass";

/**
 * A glass sheet that unfolds straight out of its trigger: down from a
 * control at the top of the screen, up from one at the bottom. No scaling
 * from a corner. Closes on outside press and Escape: an open menu left
 * over the canvas would swallow orbit drags.
 */
export function Popover({
  open,
  onClose,
  trigger,
  children,
  align = "right",
  className,
  side = "bottom",
}: {
  open: boolean;
  onClose: () => void;
  trigger: ReactNode;
  children: ReactNode;
  align?: "left" | "right" | "center";
  side?: "bottom" | "top";
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("pointerdown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open, onClose]);

  // The sheet is revealed from the edge nearest its trigger.
  const down = side === "bottom";
  const folded = down
    ? "inset(0% 0% 100% 0% round 24px)"
    : "inset(100% 0% 0% 0% round 24px)";
  const unfolded = "inset(0% 0% 0% 0% round 24px)";

  return (
    <div ref={ref} className="relative">
      {trigger}
      <AnimatePresence>
        {open && (
          <Glass
            params={GLASS.sheet}
            radius={24}
            fill={SHEET_FILL}
            initial={{ opacity: 0, y: down ? -8 : 8, clipPath: folded }}
            animate={{ opacity: 1, y: 0, clipPath: unfolded }}
            exit={{
              opacity: 0,
              y: down ? -6 : 6,
              clipPath: folded,
              transition: { duration: 0.22, ease: ease.glass },
            }}
            transition={{
              ...spring.glass,
              clipPath: { duration: 0.42, ease: ease.glass },
              opacity: { duration: 0.25 },
            }}
            className={cn(
              "absolute z-50 flex max-h-[calc(100vh-8.5rem)] flex-col p-2",
              side === "bottom" ? "top-full mt-2.5" : "bottom-full mb-2.5",
              align === "right" && "right-0",
              align === "left" && "left-0",
              align === "center" && "left-1/2 -translate-x-1/2",
              className,
            )}
          >
            <div className="scrollbar-glass relative min-h-0 overflow-y-auto">
              {children}
            </div>
          </Glass>
        )}
      </AnimatePresence>
    </div>
  );
}

/** A row inside a glass menu, with a shared sliding highlight. */
export function MenuItem({
  selected,
  onSelect,
  title,
  hint,
  icon,
  layoutGroup,
}: {
  selected?: boolean;
  onSelect: () => void;
  title: ReactNode;
  hint?: ReactNode;
  icon?: ReactNode;
  layoutGroup: string;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className="group relative flex w-full items-start gap-3 rounded-2xl px-3.5 py-2.5 text-left"
    >
      {selected && (
        <motion.span
          layoutId={`${layoutGroup}-active`}
          transition={spring.snappy}
          className="absolute inset-0 rounded-2xl bg-white/12"
        />
      )}
      <span className="absolute inset-0 rounded-2xl bg-white/0 transition-colors duration-200 group-hover:bg-white/6" />
      {icon && (
        <span className="relative mt-0.5 grid size-5 shrink-0 place-items-center text-white/80 transition-transform duration-300 ease-[var(--ease-glass)] group-hover:scale-110 [&>svg]:size-full">
          {icon}
        </span>
      )}
      <span className="relative flex min-w-0 flex-col gap-0.5">
        <span
          className={cn(
            "text-[1.0625rem] font-semibold transition-colors",
            selected ? "text-white" : "text-white/80 group-hover:text-white",
          )}
        >
          {title}
        </span>
        {hint && (
          <span className="text-[0.875rem] leading-snug font-medium text-white/50">
            {hint}
          </span>
        )}
      </span>
    </button>
  );
}
