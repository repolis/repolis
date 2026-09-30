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

/**
 * For wrappers that hold glass: movement only. Opacity or a filter on an
 * ancestor would switch the panes' refraction off until it settles.
 */
export const rise: Variants = {
  hidden: { y: 12 },
  show: { y: 0, transition: spring.glass },
  exit: { y: -6, transition: { duration: 0.2, ease: ease.glass } },
};

/** The fade a glass pane runs on itself, driven by its parent's labels. */
export const paneFade: Variants = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { duration: 0.45, ease: ease.glass } },
  exit: { opacity: 0, transition: { duration: 0.2, ease: ease.glass } },
};

/** `paneFade` after a delay, for panes that enter on their own. */
export const paneFadeAfter = (delay: number): Variants => ({
  ...paneFade,
  show: { opacity: 1, transition: { duration: 0.45, ease: ease.glass, delay } },
});

export const stagger = (step = 0.06, delay = 0): Variants => ({
  hidden: {},
  show: { transition: { staggerChildren: step, delayChildren: delay } },
  exit: { transition: { staggerChildren: step / 2, staggerDirection: -1 } },
});
