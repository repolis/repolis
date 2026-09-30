import { Outlet } from "@tanstack/react-router";
import { MotionConfig } from "motion/react";

import { CloudSky } from "@/shared/sky/cloud-sky";
import { SkyPhoto } from "@/shared/sky/sky-photo";
import { Vignette } from "@/shared/ui/brand";

/** The sky and the frame darkening persist across routes; pages
 * swap in between them, so moving from the landing page into a city never
 * cuts. */
export function RootLayout() {
  return (
    <MotionConfig reducedMotion="user">
      <div className="relative h-full w-full overflow-hidden">
        <SkyPhoto />
        <CloudSky />
        <Vignette />
        <Outlet />
      </div>
    </MotionConfig>
  );
}
