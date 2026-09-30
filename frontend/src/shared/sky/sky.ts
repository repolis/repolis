import { animate, motionValue, type MotionValue } from "motion/react";

/**
 * The sky is one layer shared by every route: it lives in the root layout, so
 * the clouds on the landing page are the same clouds the city emerges from.
 * Pages steer it through these values instead of mounting their own.
 */
export interface SkyState {
  /** Opacity of the painted sky behind the clouds. */
  sky: MotionValue<number>;
  /** Density of the full-screen cloud blanket. 0 is a clear sky. */
  fog: MotionValue<number>;
  /** 0..1: clouds slide outward and dissolve from the centre. */
  part: MotionValue<number>;
  /** Push into the clouds, as if the camera were descending. */
  zoom: MotionValue<number>;
  /** Drift speed multiplier. */
  speed: MotionValue<number>;
}

export const sky: SkyState = {
  sky: motionValue(1),
  fog: motionValue(0.14),
  part: motionValue(0),
  zoom: motionValue(0),
  speed: motionValue(1),
};

type Target = Partial<Record<keyof SkyState, number>>;

function to(target: Target, duration: number, delay = 0): Promise<void> {
  const runs = (Object.keys(target) as (keyof SkyState)[]).map((k) =>
    animate(sky[k], target[k] as number, {
      duration,
      delay,
      ease: [0.65, 0, 0.35, 1],
    }),
  );
  return Promise.all(runs).then(() => undefined);
}

export function isSkyVisible(): boolean {
  return (
    sky.sky.get() > 0.001 || (sky.fog.get() > 0.001 && sky.part.get() < 0.999)
  );
}

export const skyScenes = {
  /** Clear morning sky with a thin haze at the horizon. */
  landing: () => to({ sky: 1, fog: 0.14, part: 0, zoom: 0, speed: 1 }, 1.4),
  /** Dive into the cloud bank after a repository is submitted. */
  dive: () => to({ sky: 1, fog: 1, part: 0, zoom: 1, speed: 2.6 }, 1.1),
  /** Hold inside the clouds while the city is being built. */
  holding: () => to({ sky: 1, fog: 1, part: 0, zoom: 0.55, speed: 1.4 }, 1.2),
  /** Instantly cover the screen (deep link straight into a city). */
  cover: () => {
    sky.sky.jump(1);
    sky.fog.jump(1);
    sky.part.jump(0);
    sky.zoom.jump(0.55);
    sky.speed.jump(1.4);
  },
  /** Part the clouds and fade the sky: the city underneath takes over. */
  reveal: () =>
    Promise.all([
      to({ part: 1, zoom: 1.4, speed: 3 }, 2.6),
      to({ sky: 0 }, 1.8, 0.5),
    ]).then(() => {
      sky.fog.jump(0);
    }),
};
