import {
  forwardRef,
  useCallback,
  type CSSProperties,
  type ReactNode,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  motion,
  useReducedMotion,
  useSpring,
  type HTMLMotionProps,
} from "motion/react";

import { cn } from "@/shared/lib/cn";
import { press, spring } from "@/shared/lib/motion";
import { REFRACT_ID, useCanRefract } from "@/shared/lib/refraction";

/** Mounted once at the root: the displacement the glass rims sample. */
export function GlassFilters() {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      className="pointer-events-none absolute h-0 w-0"
    >
      <defs>
        <filter id={REFRACT_ID} colorInterpolationFilters="sRGB">
          <feTurbulence
            type="fractalNoise"
            baseFrequency="0.014 0.022"
            numOctaves={2}
            seed={7}
            result="noise"
          />
          <feGaussianBlur in="noise" stdDeviation={1.4} result="soft" />
          <feDisplacementMap
            in="SourceGraphic"
            in2="soft"
            scale={22}
            xChannelSelector="R"
            yChannelSelector="G"
          />
        </filter>
      </defs>
    </svg>
  );
}

/* ------------------------------------------------------------------------ */
/* GlassPanel: large surfaces (inspector, loader, palette, legend)           */
/* ------------------------------------------------------------------------ */

const RIM = 10;
const SPECULAR_SIZE = 280;

/** Clips a layer to a ring along the border box, so only the rim bends. */
const ringMask = (width: number): CSSProperties => ({
  boxSizing: "border-box",
  padding: width,
  maskImage: "linear-gradient(#000, #000), linear-gradient(#000, #000)",
  maskClip: "content-box, border-box",
  maskComposite: "exclude",
  WebkitMaskImage: "linear-gradient(#000, #000), linear-gradient(#000, #000)",
  WebkitMaskClip: "content-box, border-box",
  WebkitMaskComposite: "xor",
});

const CHROMATIC_RIM =
  "linear-gradient(125deg, oklch(0.86 0.1 215 / 0.35) 0%, transparent 34%, transparent 66%, oklch(0.8 0.13 340 / 0.28) 100%)";
const SPECULAR =
  "radial-gradient(closest-side, rgb(255 255 255 / 0.22), rgb(255 255 255 / 0.05) 50%, transparent 75%)";

export interface GlassPanelProps extends Omit<
  HTMLMotionProps<"div">,
  "children"
> {
  children?: ReactNode;
  /** Corner radius in rem. The Figma cards use 1.5rem (24px). */
  radius?: number;
  /** Darker body for long reading, as in the inspector. */
  strong?: boolean;
  /** Pointer-tracked specular sweep. */
  specular?: boolean;
  innerClassName?: string;
}

/**
 * Adapted from the 21st.dev "Glass Card": a frosted body inset from a
 * refractive rim that visibly bends the city behind it, with a specular
 * highlight that follows the pointer and settles back when it leaves.
 */
export const GlassPanel = forwardRef<HTMLDivElement, GlassPanelProps>(
  function GlassPanel(
    {
      children,
      className,
      innerClassName,
      radius = 1.5,
      strong = false,
      specular = true,
      style,
      onPointerMove,
      onPointerLeave,
      ...rest
    },
    ref,
  ) {
    const reduce = useReducedMotion();
    const refract = useCanRefract();
    const sx = useSpring(-SPECULAR_SIZE, spring.follow);
    const sy = useSpring(-SPECULAR_SIZE, spring.follow);
    const rx = useSpring(0, spring.follow);
    const ry = useSpring(0, spring.follow);

    const handleMove = useCallback(
      (e: ReactPointerEvent<HTMLDivElement>) => {
        onPointerMove?.(e);
        if (reduce) return;
        const r = e.currentTarget.getBoundingClientRect();
        sx.set(e.clientX - r.left - SPECULAR_SIZE / 2);
        sy.set(e.clientY - r.top - SPECULAR_SIZE / 2);
        rx.set(((e.clientX - r.left) / r.width - 0.5) * 6);
        ry.set(((e.clientY - r.top) / r.height - 0.5) * 6);
      },
      [onPointerMove, reduce, rx, ry, sx, sy],
    );

    const handleLeave = useCallback(
      (e: ReactPointerEvent<HTMLDivElement>) => {
        onPointerLeave?.(e);
        sx.set(-SPECULAR_SIZE);
        sy.set(-SPECULAR_SIZE);
        rx.set(0);
        ry.set(0);
      },
      [onPointerLeave, rx, ry, sx, sy],
    );

    const tint = strong ? "rgb(18 20 26 / 0.36)" : "rgb(18 20 26 / 0.24)";
    const bodyBlur = strong ? "1.75rem" : "1.375rem";
    const rimFilter = `blur(1px) saturate(180%) brightness(1.08)`;

    return (
      <motion.div
        ref={ref}
        className={cn("relative isolate", className)}
        style={{
          borderRadius: `${radius}rem`,
          boxShadow:
            "0 1.75rem 4rem -1.75rem rgb(0 0 0 / 0.55), 0 0.75rem 1.75rem -1.25rem rgb(0 0 0 / 0.4)",
          ...style,
        }}
        onPointerMove={handleMove}
        onPointerLeave={handleLeave}
        {...rest}
      >
        {/* Frosted body, inset so the rim samples the raw backdrop. */}
        <div
          aria-hidden
          className="absolute -z-10"
          style={{
            inset: RIM,
            borderRadius: `calc(${radius}rem - ${RIM}px)`,
            backgroundColor: tint,
            backgroundImage:
              "linear-gradient(180deg, rgb(255 255 255 / 0.08), rgb(255 255 255 / 0.02) 55%, rgb(0 0 0 / 0.06))",
            backdropFilter: `blur(${bodyBlur}) saturate(165%)`,
            WebkitBackdropFilter: `blur(${bodyBlur}) saturate(165%)`,
            boxShadow:
              "inset 0 1px 0 rgb(255 255 255 / 0.18), inset 0 0 0 1px rgb(255 255 255 / 0.05)",
          }}
        />

        {/* Refractive rim: the edge that bends what sits behind it. */}
        <div
          aria-hidden
          className="absolute inset-0 -z-10 rounded-[inherit]"
          style={{
            ...ringMask(RIM),
            backgroundColor: tint,
            backdropFilter: refract
              ? `url(#${REFRACT_ID}) ${rimFilter}`
              : rimFilter,
            WebkitBackdropFilter: rimFilter,
          }}
        />

        {/* Light, isolated so blending never leaks past the pane. */}
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 -z-10 overflow-hidden rounded-[inherit]"
        >
          <motion.div
            className="absolute inset-0 rounded-[inherit]"
            style={{
              ...ringMask(RIM),
              backgroundImage: CHROMATIC_RIM,
              mixBlendMode: "plus-lighter",
              x: rx,
              y: ry,
            }}
          />
          {specular && (
            <motion.div
              className="absolute top-0 left-0"
              style={{
                width: SPECULAR_SIZE,
                height: SPECULAR_SIZE,
                backgroundImage: SPECULAR,
                mixBlendMode: "plus-lighter",
                x: sx,
                y: sy,
              }}
            />
          )}
        </div>

        {/* Polished edge, so the rim reads as one thick pane. */}
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 rounded-[inherit]"
          style={{
            boxShadow:
              "inset 0 1px 0 rgb(255 255 255 / 0.5), inset 0 0 0 1px rgb(255 255 255 / 0.14), inset 0 -1px 0 rgb(255 255 255 / 0.1)",
          }}
        />

        <div className={cn("relative", innerClassName)}>{children}</div>
      </motion.div>
    );
  },
);

/* ------------------------------------------------------------------------ */
/* Pills and buttons                                                         */
/* ------------------------------------------------------------------------ */

type Tone = "glass" | "clear" | "primary" | "ghost";

const toneClass: Record<Tone, string> = {
  glass: "glass text-white/85 hover:text-white",
  clear: "glass glass-clear text-white/85 hover:text-white",
  primary:
    "bg-white text-[#0d0f14] shadow-[0_0.5rem_1.5rem_-0.5rem_rgb(0_0_0/0.45),inset_0_-2px_0_rgb(0_0_0/0.08),inset_0_1px_0_white] hover:bg-white",
  ghost: "text-white/65 hover:bg-white/10 hover:text-white",
};

export interface GlassButtonProps extends HTMLMotionProps<"button"> {
  tone?: Tone;
  size?: "sm" | "md" | "lg";
  icon?: ReactNode;
  active?: boolean;
}

const sizeClass = {
  sm: "h-8 gap-1.5 px-3 text-[0.8125rem]",
  md: "h-10 gap-2 px-4 text-[0.9375rem]",
  lg: "h-12 gap-2 px-[1.125rem] text-[1.125rem]",
} as const;

export const GlassButton = forwardRef<HTMLButtonElement, GlassButtonProps>(
  function GlassButton(
    {
      tone = "glass",
      size = "md",
      icon,
      active,
      className,
      children,
      disabled,
      ...rest
    },
    ref,
  ) {
    return (
      <motion.button
        ref={ref}
        type="button"
        disabled={disabled}
        {...(disabled ? {} : press)}
        className={cn(
          "inline-flex shrink-0 items-center justify-center rounded-full font-semibold whitespace-nowrap transition-[color,background-color,opacity] duration-300 select-none disabled:cursor-not-allowed disabled:opacity-45",
          sizeClass[size],
          toneClass[tone],
          active && "bg-white/20 text-white",
          className,
        )}
        {...rest}
      >
        {icon && (
          <span className="grid size-[1.25em] shrink-0 place-items-center [&>svg]:size-full">
            {icon}
          </span>
        )}
        {children as ReactNode}
      </motion.button>
    );
  },
);

export const IconButton = forwardRef<
  HTMLButtonElement,
  Omit<GlassButtonProps, "icon"> & { label: string }
>(function IconButton(
  { tone = "glass", size = "md", label, className, children, ...rest },
  ref,
) {
  const dim = { sm: "size-8", md: "size-10", lg: "size-12" }[size];
  return (
    <GlassButton
      ref={ref}
      tone={tone}
      size={size}
      aria-label={label}
      title={label}
      className={cn(dim, "px-0 [&_svg]:size-[1.25rem]", className)}
      {...rest}
    >
      {children}
    </GlassButton>
  );
});

/** The Figma stat pill: icon, a bright number, a quieter label. */
export function StatPill({
  icon,
  value,
  label,
  className,
  tone = "clear",
}: {
  icon: ReactNode;
  value: ReactNode;
  label: string;
  className?: string;
  /**
   * `clear` is the Figma HUD pill, `dark` holds up over bright clouds and
   * `inset` is the lighter wash used inside a card.
   */
  tone?: "clear" | "dark" | "inset";
}) {
  return (
    <div
      className={cn(
        "inline-flex h-12 items-center gap-2 rounded-full px-[1.125rem] text-[1.25rem] leading-none font-semibold whitespace-nowrap",
        tone === "inset" && "glass-inset",
        tone === "clear" && "glass glass-clear",
        tone === "dark" && "glass",
        className,
      )}
    >
      <span className="grid size-[1.375rem] place-items-center text-white [&>svg]:size-full">
        {icon}
      </span>
      <span className="text-white tabular-nums">{value}</span>
      <span className="text-white/65">{label}</span>
    </div>
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
        "inline-grid h-[1.375rem] min-w-[1.375rem] place-items-center rounded-md bg-white/12 px-1.5 font-sans text-[0.75rem] font-semibold text-white/70 shadow-[inset_0_1px_0_rgb(255_255_255/0.2)]",
        className,
      )}
    >
      {children}
    </kbd>
  );
}

/** A small tappable tag used in lists of names. */
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
          className="size-2 shrink-0 rounded-full shadow-[0_0_0_2px_rgb(255_255_255/0.12)]"
          style={{ background: color }}
        />
      )}
      <span className="truncate">{children}</span>
    </>
  );
  const cls = cn(
    "inline-flex max-w-full min-w-0 items-center gap-1.5 rounded-full px-2.5 py-1 text-[0.8125rem] leading-tight transition-colors duration-200",
    mono && "font-mono text-[0.75rem]",
    active
      ? "bg-white text-[#0d0f14]"
      : "bg-white/10 text-white/80 shadow-[inset_0_1px_0_rgb(255_255_255/0.12)]",
    onClick && !active && "hover:bg-white/20 hover:text-white",
    className,
  );
  if (!onClick) return <span className={cls}>{body}</span>;
  return (
    <motion.button
      type="button"
      onClick={onClick}
      whileTap={{ scale: 0.95 }}
      transition={spring.snappy}
      className={cls}
    >
      {body}
    </motion.button>
  );
}
