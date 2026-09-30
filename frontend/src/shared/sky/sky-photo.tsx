import { useEffect, useState } from "react";
import { motion, useSpring, useTransform } from "motion/react";

import night from "@/assets/landing/night.webp";

import { markPhotoReady, sky } from "./sky";

/**
 * The moonlit landscape behind the landing page. It sits under the cloud
 * layer, so on submit the clouds roll in over it while it pushes toward the
 * viewer with the dive; by the time the city loads it is hidden.
 */
export function SkyPhoto() {
  const [loaded, setLoaded] = useState(false);
  const scale = useTransform(sky.zoom, (z) => 1.04 + z * 0.12);
  const visibility = useTransform(sky.photo, (o) =>
    o > 0.001 ? "visible" : "hidden",
  );
  // A few pixels of parallax: enough to feel like depth, never like motion.
  const x = useSpring(0, { stiffness: 40, damping: 20 });
  const y = useSpring(0, { stiffness: 40, damping: 20 });

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const onMove = (e: PointerEvent) => {
      x.set(-((e.clientX / window.innerWidth) * 2 - 1) * 12);
      y.set(-((e.clientY / window.innerHeight) * 2 - 1) * 8);
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    return () => window.removeEventListener("pointermove", onMove);
  }, [x, y]);

  return (
    <motion.div
      aria-hidden
      className="pointer-events-none fixed inset-0 z-10 overflow-hidden bg-[var(--color-night)]"
      style={{ opacity: sky.photo, visibility }}
    >
      <motion.img
        src={night}
        alt=""
        decoding="async"
        onLoad={() => {
          setLoaded(true);
          markPhotoReady();
        }}
        className="h-full w-full object-cover object-[50%_40%] transition-opacity duration-700"
        style={{ scale, x, y, opacity: loaded ? 1 : 0 }}
      />
    </motion.div>
  );
}
