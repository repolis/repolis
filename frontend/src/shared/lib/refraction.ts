import { useSyncExternalStore } from "react";

/** Id of the SVG displacement the glass rims sample (see GlassFilters). */
export const REFRACT_ID = "lg-refract";

/**
 * Only Chromium composites an SVG displacement inside `backdrop-filter`.
 * Everywhere else the rim keeps its blur and simply does not bend.
 */
function detectRefraction(): boolean {
  if (typeof CSS === "undefined" || typeof CSS.supports !== "function") {
    return false;
  }
  const probe = `url(#${REFRACT_ID}) blur(2px)`;
  return (
    CSS.supports("backdrop-filter", probe) ||
    CSS.supports("-webkit-backdrop-filter", probe)
  );
}

const canRefract = detectRefraction();
const subscribeNever = () => () => {};

export function useCanRefract(): boolean {
  return useSyncExternalStore(
    subscribeNever,
    () => canRefract,
    () => false,
  );
}
