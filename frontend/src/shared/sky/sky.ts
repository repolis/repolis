import { animate, motionValue, type MotionValue } from "motion/react";

/**
 * The sky is one layer shared by every route: it lives in the root layout, so
 * the clouds on the landing page are the same clouds the city emerges from.
 * Pages steer it through these values instead of mounting their own.
 */
export interface SkyState {
  /** Opacity of the night landscape photograph under the landing page. */
  photo: MotionValue<number>;
  /** Opacity of the painted sky behind the clouds. */
  sky: MotionValue<number>;
  /** Opacity of the drifting clouds high in the sky (not the blanket). */
  clouds: MotionValue<number>;
  /** Distant wisps and mist drawn over the landing photograph. */
  far: MotionValue<number>;
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
  photo: motionValue(0),
  sky: motionValue(1),
  clouds: motionValue(1),
  far: motionValue(0),
  fog: motionValue(0.14),
  part: motionValue(0),
  zoom: motionValue(0),
  speed: motionValue(1),
};

type Target = Partial<Record<keyof SkyState, number>>;

function to(
  target: Target,
  duration: number,
  delay = 0,
  ease: [number, number, number, number] = [0.65, 0, 0.35, 1],
): Promise<void> {
  const runs = (Object.keys(target) as (keyof SkyState)[]).map((k) =>
    animate(sky[k], target[k] as number, { duration, delay, ease }),
  );
  return Promise.all(runs).then(() => undefined);
}

export function isSkyVisible(): boolean {
  const covered =
    sky.fog.get() > 0.001 || sky.clouds.get() > 0.001 || sky.far.get() > 0.001;
  return sky.sky.get() > 0.001 || (covered && sky.part.get() < 0.999);
}

/*
 * One continuous camera move from the landing to the city: the push into
 * the clouds (zoom) only ever grows, and each scene starts from wherever the
 * previous one is, so there is never a jump or a reversal. A newer scene
 * cancels a queued one, so a city that arrives mid-dive is not covered again.
 */
let scene = 0;
let diving: Promise<void> | null = null;

export const skyScenes = {
  /** The moonlit landscape, clear, with nothing drawn over it. */
  landing: () => {
    scene++;
    return to(
      {
        photo: 1,
        far: 1,
        sky: 0,
        clouds: 0,
        fog: 0,
        part: 0,
        zoom: 0,
        speed: 1,
      },
      1.4,
    );
  },
  /** Push into the landscape as the cloud bank rolls in over it. */
  dive: () => {
    scene++;
    const run = Promise.all([
      to(
        { clouds: 1, fog: 1, part: 0, zoom: 0.7, speed: 2.4 },
        1.2,
        0,
        [0.5, 0, 0.75, 0.6],
      ),
      // The painted sky only takes over once the clouds hide the photo.
      to({ sky: 1 }, 0.6, 0.6),
    ]).then(() => undefined);
    diving = run.finally(() => {
      if (diving === run) diving = null;
    });
    return run;
  },
  /** Drift inside the clouds, slowing down, while the city is built. */
  holding: async () => {
    const id = ++scene;
    if (diving) await diving;
    if (id !== scene) return;
    sky.photo.jump(0);
    sky.far.jump(0);
    await to(
      { sky: 1, clouds: 1, fog: 1, part: 0, zoom: 0.95, speed: 1.2 },
      2.4,
      0,
      [0.22, 1, 0.36, 1],
    );
  },
  /** Instantly cover the screen (deep link straight into a city). */
  cover: () => {
    scene++;
    sky.photo.jump(0);
    sky.far.jump(0);
    sky.sky.jump(1);
    sky.clouds.jump(1);
    sky.fog.jump(1);
    sky.part.jump(0);
    sky.zoom.jump(0.7);
    sky.speed.jump(1.2);
  },
  /** Part the clouds and fade the sky: the city underneath takes over. */
  reveal: async () => {
    const id = ++scene;
    await Promise.all([
      to({ part: 1, zoom: 1.5, speed: 2.6 }, 2.8, 0, [0.45, 0, 0.2, 1]),
      to({ sky: 0 }, 2.0, 0.4, [0.33, 0, 0.2, 1]),
    ]);
    if (id === scene) sky.fog.jump(0);
  },
  /** True while the dive started on the landing page is still running. */
  isDiving: () => diving !== null,
};
