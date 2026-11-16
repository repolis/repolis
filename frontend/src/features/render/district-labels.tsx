import { useEffect, useRef } from "react";

import { label_positions } from "@/wasm/engine";

import type { DistrictInfo } from "./types";

/** District names over the canvas. The engine projects them; this moves the
 * elements every frame without re-rendering. */
export function DistrictLabels({ districts }: { districts: DistrictInfo[] }) {
  const refs = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    let frame = 0;
    const tick = () => {
      const p = label_positions();
      refs.current.forEach((el, i) => {
        if (!el) return;
        const alpha = p[i * 3 + 2] ?? 0;
        el.style.opacity = String(alpha);
        if (alpha > 0)
          el.style.transform = `translate(${p[i * 3]}px, ${p[i * 3 + 1]}px) translateX(-50%)`;
      });
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);

  return districts.map((d, i) => (
    <div
      key={d.id}
      ref={(el) => {
        refs.current[i] = el;
      }}
      className="pointer-events-none absolute top-0 left-0 z-30 font-mono text-[13px] whitespace-nowrap text-[#fffcf0] opacity-0 [text-shadow:0_1px_2px_rgb(0_0_0/0.6)]"
    >
      {d.name} ({d.building_count})
    </div>
  ));
}
