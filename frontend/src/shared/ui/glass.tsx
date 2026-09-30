import {
  forwardRef,
  useImperativeHandle,
  type ReactNode,
  type Ref,
} from "react";
import { motion, type HTMLMotionProps } from "motion/react";

import { GLASS, type GlassParams } from "@/shared/glass/maps";
import { useGlass, type GlassRadius } from "@/shared/glass/use-glass";
import { cn } from "@/shared/lib/cn";
import { paneFade, spring } from "@/shared/lib/motion";

/*
 * Every surface in the interface is one of these. No drop shadows anywhere:
 * legibility comes from the darkened frame edges, the glass itself and the
 * type, not from shading behind it.
 *
 * Panes fade themselves in (`paneFade`, picked up from a parent's variant
 * labels). A wrapper must never fade them: an ancestor with opacity below 1
 * or a filter cuts the glass off from the scene until it settles, and the
 * pane pops from flat to glass. A pane that mounts later, after its
 * parent's labels have run, needs `inherit={false}` or it stays hidden.
 */

/** Hands the element to a forwarded ref once the glass hook has it. */
function useForward<T>(forwarded: Ref<T> | undefined, el: T | null) {
  useImperativeHandle(forwarded, () => el as T, [el]);
}

export interface GlassProps extends Omit<HTMLMotionProps<"div">, "children"> {
  children?: ReactNode;
  params?: GlassParams;
  radius?: GlassRadius;
  /** Tint of the pane body. Figma: 5% on pills, 1% on the card. */
  fill?: string;
}

/** The darker tint for sheets that hold text: menus, the palette, the loader. */
export const SHEET_FILL = "rgb(20 24 32 / 0.35)";

/** A pane of liquid glass. */
export const Glass = forwardRef<HTMLDivElement, GlassProps>(function Glass(
  {
    children,
    params = GLASS.pill,
    radius = "full",
    fill = "rgb(255 255 255 / 0.05)",
    className,
    style,
    variants = paneFade,
    ...rest
  },
  forwarded,
) {
  const {
    attach,
    el,
    style: glassStyle,
    layers,
  } = useGlass<HTMLDivElement>(params, radius);
  useForward(forwarded, el);
  return (
    <motion.div
      ref={attach}
      data-glass=""
      variants={variants}
      className={cn("relative isolate", className)}
      style={{
        borderRadius: radius === "full" ? 9999 : `${radius / 16}rem`,
        backgroundColor: fill,
        ...glassStyle,
        ...style,
      }}
      {...rest}
    >
      {layers}
      {children}
    </motion.div>
  );
});

/* ------------------------------------------------------------------------ */
/* Controls: one height (48px in the frame), one radius, one type size.      */
/* ------------------------------------------------------------------------ */

/**
 * `inset` is the flat 15% pill from the Figma card: a control that sits on
 * another pane of glass cannot have its own, since the pane it sits on is
 * all its backdrop filter would see.
 */
type Tone = "glass" | "primary" | "ghost" | "inset";
type Size = "hud" | "sm";

function controlClass(tone: Tone, size: Size, active?: boolean) {
  return cn(
    "glass-control relative isolate inline-flex shrink-0 items-center justify-center gap-2 rounded-full whitespace-nowrap transition-[background-color,color] duration-300 ease-[var(--ease-glass)] select-none disabled:cursor-not-allowed disabled:opacity-40",
    size === "hud"
      ? "type-hud h-12 px-[1.125rem]"
      : "h-9 px-3.5 text-[0.9375rem]",
    tone === "glass" &&
      (active
        ? "bg-white/15 text-white"
        : "bg-white/5 text-white/85 hover:bg-white/[0.09] hover:text-white"),
    tone === "primary" && "bg-white text-[#0d0f14] hover:bg-white/90",
    tone === "ghost" &&
      "bg-transparent text-white/65 hover:bg-white/10 hover:text-white",
    tone === "inset" &&
      (active
        ? "bg-white/25 text-white"
        : "bg-white/15 text-white/85 hover:bg-white/20 hover:text-white"),
  );
}

function ControlBody({
  icon,
  size,
  children,
}: {
  icon?: ReactNode;
  size: Size;
  children?: ReactNode;
}) {
  return (
    <>
      {icon && (
        <span
          className={cn(
            "glass-icon relative grid shrink-0 place-items-center [&>svg]:size-full",
            size === "hud" ? "size-[1.375rem]" : "size-[1.125rem]",
          )}
        >
          {icon}
        </span>
      )}
      {children !== undefined && children !== null && (
        <span className="relative inline-flex items-center gap-2">
          {children}
        </span>
      )}
    </>
  );
}

export interface GlassButtonProps extends HTMLMotionProps<"button"> {
  tone?: Tone;
  icon?: ReactNode;
  active?: boolean;
  /** 48px HUD control (default) or a 36px control inside panels. */
  size?: Size;
}

export const GlassButton = forwardRef<HTMLButtonElement, GlassButtonProps>(
  function GlassButton(
    {
      tone = "glass",
      size = "hud",
      icon,
      active,
      className,
      children,
      disabled,
      style,
      variants = paneFade,
      ...rest
    },
    forwarded,
  ) {
    const {
      attach,
      el,
      style: glassStyle,
      layers,
    } = useGlass<HTMLButtonElement>(GLASS.pill, "full");
    useForward(forwarded, el);
    const isGlass = tone === "glass";
    return (
      <motion.button
        ref={attach}
        type="button"
        data-glass={isGlass ? "" : undefined}
        disabled={disabled}
        variants={variants}
        whileTap={disabled ? undefined : { scale: 0.96 }}
        transition={spring.snappy}
        className={cn(controlClass(tone, size, active), className)}
        style={isGlass ? { ...glassStyle, ...style } : style}
        {...rest}
      >
        {isGlass && layers}
        <ControlBody icon={icon} size={size}>
          {children as ReactNode}
        </ControlBody>
      </motion.button>
    );
  },
);

export interface GlassLinkProps extends HTMLMotionProps<"a"> {
  tone?: Tone;
  icon?: ReactNode;
  size?: Size;
}

/** A glass control that navigates: a real link, so it opens in a new tab,
 * shows its address and works with the keyboard like any other. */
export const GlassLink = forwardRef<HTMLAnchorElement, GlassLinkProps>(
  function GlassLink(
    {
      tone = "glass",
      size = "hud",
      icon,
      className,
      children,
      style,
      variants = paneFade,
      ...rest
    },
    forwarded,
  ) {
    const {
      attach,
      el,
      style: glassStyle,
      layers,
    } = useGlass<HTMLAnchorElement>(GLASS.pill, "full");
    useForward(forwarded, el);
    const isGlass = tone === "glass";
    return (
      <motion.a
        ref={attach}
        target="_blank"
        rel="noreferrer"
        data-glass={isGlass ? "" : undefined}
        variants={variants}
        whileTap={{ scale: 0.96 }}
        transition={spring.snappy}
        className={cn(controlClass(tone, size), className)}
        style={isGlass ? { ...glassStyle, ...style } : style}
        {...rest}
      >
        {isGlass && layers}
        <ControlBody icon={icon} size={size}>
          {children as ReactNode}
        </ControlBody>
      </motion.a>
    );
  },
);

export const IconButton = forwardRef<
  HTMLButtonElement,
  Omit<GlassButtonProps, "icon"> & { label: string }
>(function IconButton(
  { label, className, children, size = "hud", ...rest },
  ref,
) {
  return (
    <GlassButton
      ref={ref}
      size={size}
      aria-label={label}
      title={label}
      className={cn(
        size === "hud" ? "size-12" : "size-9",
        "glass-icon px-0 [&_svg]:size-[1.375rem]",
        className,
      )}
      {...rest}
    >
      {children}
    </GlassButton>
  );
});

/** The Figma stat pill: icon, a white number, a quieter label. */
export function StatPill({
  icon,
  value,
  label,
  className,
  tone = "glass",
}: {
  icon: ReactNode;
  value: ReactNode;
  label: string;
  className?: string;
  /** `inset` is the 15% wash used inside the card, without its own glass. */
  tone?: "glass" | "inset";
}) {
  const body = (
    <>
      <span className="relative grid size-[1.375rem] place-items-center [&>svg]:size-full">
        {icon}
      </span>
      <span className="relative whitespace-nowrap">
        <span className="text-white tabular-nums">{value}</span>
        <span className="text-white/85"> </span>
        <span className="text-white/65">{label}</span>
      </span>
    </>
  );
  const cls = cn(
    "type-hud inline-flex h-12 items-center gap-2 rounded-full px-[1.125rem]",
    className,
  );
  if (tone === "inset") {
    return <div className={cn(cls, "bg-white/15")}>{body}</div>;
  }
  return (
    <Glass className={cls} fill="rgb(255 255 255 / 0.05)">
      {body}
    </Glass>
  );
}

export function Kbd({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <kbd
      className={cn(
        "inline-grid h-6 min-w-6 place-items-center rounded-md bg-white/10 px-1.5 font-sans text-[0.8125rem] font-semibold text-white/65",
        className,
      )}
    >
      {children}
    </kbd>
  );
}

/** A flat tag for lists of names inside panels. */
export function Chip({
  children,
  onClick,
  active,
  mono,
  className,
  color,
}: {
  children: ReactNode;
  onClick?: () => void;
  active?: boolean;
  mono?: boolean;
  className?: string;
  color?: string;
}) {
  const body = (
    <>
      {color && (
        <span
          className="size-2 shrink-0 rounded-full"
          style={{ background: color }}
        />
      )}
      <span className="truncate">{children}</span>
    </>
  );
  const cls = cn(
    "inline-flex max-w-full min-w-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-[0.9375rem] leading-tight transition-colors duration-200",
    mono && "font-mono text-[0.8125rem] font-medium",
    active ? "bg-white text-[#0d0f14]" : "bg-white/10 text-white/80",
    onClick && !active && "hover:bg-white/15 hover:text-white",
    className,
  );
  if (!onClick) return <span className={cls}>{body}</span>;
  return (
    <motion.button
      type="button"
      onClick={onClick}
      whileTap={{ scale: 0.96 }}
      transition={spring.snappy}
      className={cls}
    >
      {body}
    </motion.button>
  );
}
