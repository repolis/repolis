import { useEffect, useEffectEvent } from "react";

import type { EngineTarget } from "./city";

export function useEngineEvents(handlers: {
  hover: (target: EngineTarget) => void;
  select: (target: EngineTarget) => void;
  city: () => void;
}) {
  const hover = useEffectEvent(handlers.hover);
  const select = useEffectEvent(handlers.select);
  const city = useEffectEvent(handlers.city);

  useEffect(() => {
    const target = (e: Event) => (e as CustomEvent<EngineTarget>).detail;
    const listeners: [string, (e: Event) => void][] = [
      ["repolis:hover", (e) => hover(target(e))],
      ["repolis:select", (e) => select(target(e))],
      ["repolis:city", () => city()],
    ];
    for (const [name, fn] of listeners) window.addEventListener(name, fn);
    return () => {
      for (const [name, fn] of listeners) window.removeEventListener(name, fn);
    };
  }, []);
}
