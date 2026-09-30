import { useEffect, useRef, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { spring } from "@/shared/lib/motion";

/**
 * A glass sheet that grows out of its trigger. Closes on outside press and
 * Escape: an open menu left over the canvas would swallow orbit drags.
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

  const origin =
    (side === "bottom" ? "top " : "bottom ") +
    (align === "center" ? "center" : align);

  return (
    <div ref={ref} className="relative">
      {trigger}
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{
              opacity: 0,
              scale: 0.92,
              y: side === "bottom" ? -8 : 8,
            }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: side === "bottom" ? -4 : 4 }}
            transition={spring.glass}
            style={{ transformOrigin: origin }}
            className={cn(
              "glass glass-strong absolute z-50 rounded-[1.25rem] p-1.5",
              side === "bottom" ? "top-full mt-3" : "bottom-full mb-3",
              align === "right" && "right-0",
              align === "left" && "left-0",
              align === "center" && "left-1/2 -translate-x-1/2",
              className,
            )}
          >
            {children}
          </motion.div>
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
          className="absolute inset-0 rounded-2xl bg-white/14 shadow-[inset_0_1px_0_rgb(255_255_255/0.18)]"
        />
      )}
      <span className="absolute inset-0 rounded-2xl bg-white/0 transition-colors duration-200 group-hover:bg-white/7" />
      {icon && (
        <span className="relative mt-0.5 grid size-5 shrink-0 place-items-center text-white/80 [&>svg]:size-full">
          {icon}
        </span>
      )}
      <span className="relative flex min-w-0 flex-col gap-0.5">
        <span
          className={cn(
            "text-[0.9375rem] font-semibold transition-colors",
            selected ? "text-white" : "text-white/80 group-hover:text-white",
          )}
        >
          {title}
        </span>
        {hint && (
          <span className="text-[0.8125rem] leading-snug text-white/50">
            {hint}
          </span>
        )}
      </span>
    </button>
  );
}
