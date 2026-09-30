import { Outlet } from "@tanstack/react-router";
import { MotionConfig } from "motion/react";

import { CloudSky } from "@/shared/sky/cloud-sky";
import { Corners, Vignette } from "@/shared/ui/brand";
import { GlassFilters } from "@/shared/ui/glass";
import { PointerLight } from "@/shared/ui/pointer-light";

/** The sky, the lens and the light persist across routes; pages swap in
 * between them, so moving from the landing page into a city never cuts. */
export function RootLayout() {
  return (
    <MotionConfig reducedMotion="user">
      <div className="relative h-full w-full overflow-hidden">
        <GlassFilters />
        <CloudSky />
        <Vignette />
        <Corners />
        <PointerLight />
        <Outlet />
      </div>
    </MotionConfig>
  );
}
