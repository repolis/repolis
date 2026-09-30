import type { Transition, Variants } from "motion/react";

/**
 * One motion vocabulary for the whole interface, so every panel settles the
 * same way. Springs are critically damped or close to it: glass should land,
 * not wobble.
 */
export const spring = {
  /** Buttons, toggles, anything under the finger. */
  snappy: { type: "spring", stiffness: 520, damping: 34, mass: 0.7 },
  /** Panels entering and leaving. */
  glass: { type: "spring", stiffness: 260, damping: 30, mass: 0.9 },
  /** Large, slow moves: the loader morphing, the HUD settling. */
  soft: { type: "spring", stiffness: 140, damping: 24, mass: 1 },
  /** Things that follow the pointer. */
  follow: { type: "spring", stiffness: 380, damping: 32, mass: 0.4 },
} satisfies Record<string, Transition>;

export const ease = {
  glass: [0.22, 1, 0.36, 1],
  inOut: [0.65, 0, 0.35, 1],
} as const;

/** Blur-in reveal: content condenses out of the glass rather than sliding. */
export const reveal: Variants = {
  hidden: { opacity: 0, y: 12, filter: "blur(10px)" },
  show: {
    opacity: 1,
    y: 0,
    filter: "blur(0px)",
    transition: { ...spring.glass, opacity: { duration: 0.35 } },
    // A lingering `blur(0px)` would make this element a backdrop root and
    // switch off the glass blur of everything inside it.
    transitionEnd: { filter: "none" },
  },
  exit: {
    opacity: 0,
    y: -6,
    filter: "blur(8px)",
    transition: { duration: 0.2, ease: ease.glass },
  },
};

export const stagger = (step = 0.06, delay = 0): Variants => ({
  hidden: {},
  show: { transition: { staggerChildren: step, delayChildren: delay } },
  exit: { transition: { staggerChildren: step / 2, staggerDirection: -1 } },
});

/**
 * Blur-in for anything that contains glass. `filter` must end as `none`: any
 * other value, even blur(0px), stops descendants' backdrop-filter from
 * seeing the city.
 */
export const glassIn = {
  initial: { opacity: 0, scale: 0.96, filter: "blur(10px)" },
  animate: {
    opacity: 1,
    scale: 1,
    filter: "blur(0px)",
    transitionEnd: { filter: "none" },
  },
  exit: { opacity: 0, scale: 0.97, filter: "blur(8px)" },
  transition: spring.glass,
} as const;

/** Press feedback shared by every glass control. */
export const press = {
  whileHover: { scale: 1.03 },
  whileTap: { scale: 0.96 },
  transition: spring.snappy,
} as const;
