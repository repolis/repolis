import { useEffect } from "react";

/**
 * Publishes the pointer position as CSS variables on the root element. Every
 * glass surface reads them for its specular highlight, so the whole interface
 * shares one light source that follows the cursor.
 */
export function PointerLight() {
  useEffect(() => {
    const root = document.documentElement;
    let raf = 0;
    let x = -1000;
    let y = -1000;
    const flush = () => {
      raf = 0;
      root.style.setProperty("--pointer-x", `${x}px`);
      root.style.setProperty("--pointer-y", `${y}px`);
    };
    const onMove = (e: PointerEvent) => {
      x = e.clientX;
      y = e.clientY;
      if (!raf) raf = requestAnimationFrame(flush);
    };
    const onLeave = () => {
      x = -1000;
      y = -1000;
      if (!raf) raf = requestAnimationFrame(flush);
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    document.addEventListener("pointerleave", onLeave);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerleave", onLeave);
    };
  }, []);
  return null;
}
