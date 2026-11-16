import { useEffect } from "react";

import { camera_state } from "@/wasm/engine";

import { encodeView } from "./url-state";
import type { FilterState } from "./view-controls";

/**
 * Keeps the URL fragment in step with the view. replaceState, and the camera
 * only polled, so orbiting does not bury the back button in history entries.
 */
export function useUrlSync(
  active: boolean,
  mode: string,
  filter: FilterState,
  selected: string | null,
  timeline: number | null,
) {
  useEffect(() => {
    if (!active) return;
    const write = () => {
      const base = window.location.pathname + window.location.search;
      const camera = camera_state() || null;
      const next =
        base + encodeView({ mode, filter, selected, timeline, camera });
      if (next !== base + window.location.hash)
        window.history.replaceState(null, "", next);
    };
    write();
    const id = window.setInterval(write, 700);
    return () => window.clearInterval(id);
  }, [active, mode, filter, selected, timeline]);
}
