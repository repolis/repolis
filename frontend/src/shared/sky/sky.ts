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

/*
 * Going back to the landing page is a full page load (the engine binds to
 * its canvas once). To keep that seamless, the city page leaves a hand-off
 * note: the clouds' clock and camera push at the moment it closed them. The
 * new page starts inside exactly those clouds and opens them from there.
 */
const HANDOFF_KEY = "repolis:sky-handoff";

interface Handoff {
  clock: number;
  zoom: number;
  speed: number;
}

function readHandoff(): Handoff | null {
  try {
    const raw = window.sessionStorage.getItem(HANDOFF_KEY);
    return raw ? (JSON.parse(raw) as Handoff) : null;
  } catch {
    return null;
  }
}

const handoff = typeof window === "undefined" ? null : readHandoff();

/** True when this page load continues a flight back from a city. */
export const returning = handoff !== null;

/** The page starts covered when it opens on a city or comes back from one;
 * otherwise straight on the clear landscape, with nothing to fade through. */
const covered =
  returning ||
  (typeof window !== "undefined" &&
    window.location.pathname.startsWith("/city"));

export const sky: SkyState = {
  photo: motionValue(covered ? 0 : 1),
  sky: motionValue(covered ? 1 : 0),
  clouds: motionValue(covered ? 1 : 0),
  far: motionValue(0),
  fog: motionValue(covered ? 1 : 0),
  part: motionValue(0),
  zoom: motionValue(handoff?.zoom ?? (covered ? 0.7 : 0)),
  speed: motionValue(handoff?.speed ?? (covered ? 1.2 : 1)),
};

/** The clouds' clock, written by the renderer every frame. */
export const skyClock = { value: handoff?.clock ?? 0 };

let photoLoaded: () => void = () => {};
const photoReady = new Promise<void>((resolve) => {
  photoLoaded = resolve;
});
/** Called by the photograph layer once its image has decoded. */
export function markPhotoReady() {
  photoLoaded();
}

/** Where the camera rests when the clouds are closed over a city. */
const CLOSED = { zoom: 0.95, speed: 1.2 } as const;

const wait = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

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
  /** The moonlit landscape, clear, with distant clouds drifting in. */
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
  /**
   * Back from a city: the page opened inside the clouds the city closed,
   * and they now part from the centre onto the landscape while the camera
   * rises out of them.
   */
  arrive: async () => {
    const id = ++scene;
    // Open onto the photograph, not onto its empty frame.
    await Promise.race([photoReady, wait(1500)]);
    if (id !== scene) return;
    sky.photo.jump(1);
    await Promise.all([
      to({ part: 1, zoom: 0, speed: 1 }, 2.6, 0, [0.45, 0, 0.2, 1]),
      to({ sky: 0 }, 1.8, 0.3, [0.33, 0, 0.2, 1]),
    ]);
    if (id !== scene) return;
    // Everything below is invisible at part 1; reset it to the landing.
    sky.fog.jump(0);
    sky.clouds.jump(0);
    sky.part.jump(0);
    await to({ far: 1 }, 1.6);
  },
  /** Close the clouds over the city, from the edges in, before leaving. */
  closeOver: async () => {
    ++scene;
    await to(
      { sky: 1, clouds: 1, fog: 1, part: 0, ...CLOSED },
      1.1,
      0,
      [0.55, 0, 0.35, 1],
    );
  },
  /** Leave the note the next page load picks the clouds up from. */
  handOff: () => {
    try {
      // The closed state, not the live values: a slow frame must not hand
      // over a camera that is still mid-move.
      window.sessionStorage.setItem(
        HANDOFF_KEY,
        JSON.stringify({ clock: skyClock.value, ...CLOSED } satisfies Handoff),
      );
    } catch {
      // Private mode: the return simply starts from fresh clouds.
    }
  },
  /** The landing page has picked the hand-off up; the clouds are drawn by
   * now, so the stand-in ground from index.html can go too. */
  consumeHandOff: () => {
    document.documentElement.classList.remove("sky-return");
    try {
      window.sessionStorage.removeItem(HANDOFF_KEY);
    } catch {
      // Nothing to clear.
    }
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
