/**
 * Liquid glass, modelled on Figma's GLASS effect.
 *
 * The pane is a slab with a rounded bevel along its edge. Light from the
 * city passes through it: on the flat top it goes straight through, on the
 * bevel it is refracted (Snell's law, n = 1.5) and lands displaced toward
 * the centre, which bends the backdrop at the rim the way thick glass does.
 * The same bevel normals give the specular rim that catches the light.
 *
 * Both are baked into small images per size: a displacement map fed to an
 * SVG feDisplacementMap, and a specular overlay drawn on top.
 */

export interface GlassParams {
  /** Bevel width in px: how far the curved edge reaches into the pane. */
  depth: number;
  /** 0..1, strength of the bend. */
  refraction: number;
  /** 0..1, how far red and blue separate at the rim. */
  dispersion: number;
  /** Backdrop blur radius in px (Figma "frost"). */
  frost: number;
  /** Degrees; the direction light arrives from (Figma convention). */
  lightAngle: number;
  /** 0..1, brightness of the specular rim. */
  lightIntensity: number;
  /** 0..1, how far the highlight spreads around the rim. */
  splay: number;
}

/** Values read from the Figma file (effects on the HUD layers). */
export const GLASS = {
  /** Stat pills and every other 48px control. */
  pill: {
    depth: 10,
    refraction: 0.8,
    dispersion: 0.5,
    frost: 2,
    lightAngle: -45,
    lightIntensity: 0.8,
    splay: 0,
  },
  /** The inspector card. */
  card: {
    depth: 75,
    refraction: 0.8,
    dispersion: 0.5,
    frost: 33,
    lightAngle: -45,
    lightIntensity: 0.8,
    splay: 1,
  },
  /** Menus and the command palette: card glass, shallower bevel. */
  sheet: {
    depth: 36,
    refraction: 0.8,
    dispersion: 0.5,
    frost: 33,
    lightAngle: -45,
    lightIntensity: 0.8,
    splay: 1,
  },
  /** The landing capsule: a thick pill that lenses the scene at its rim
   * and is frosted enough in the middle for the text to sit on calm glass. */
  capsule: {
    depth: 22,
    refraction: 1,
    dispersion: 0.6,
    frost: 28,
    lightAngle: -45,
    lightIntensity: 0.9,
    splay: 0.3,
  },
} satisfies Record<string, GlassParams>;

export interface GlassMaps {
  displacement: string;
  specular: string;
  /** feDisplacementMap scale that turns the encoded map into pixels. */
  scale: number;
}

const IOR = 1.5;

/** Convex squircle bevel: steep at the outer edge, flat where it meets the top. */
function bevelSlope(t: number): number {
  const u = 1 - t;
  const inner = 1 - u ** 4;
  if (inner <= 1e-6) return 50;
  return u ** 3 / inner ** 0.75;
}

/** Lateral displacement at bevel position t (0 = rim, 1 = flat), 0..~1. */
function bend(t: number): number {
  const theta = Math.atan(bevelSlope(Math.max(t, 0.002)));
  const refracted = Math.asin(Math.sin(theta) / IOR);
  return Math.tan(theta - refracted);
}

const BEND_MAX = bend(0.002);

/**
 * Signed distance to a rounded rectangle centred at the origin, and the
 * outward normal of the nearest edge.
 */
function roundedRect(
  px: number,
  py: number,
  hw: number,
  hh: number,
  r: number,
): { d: number; nx: number; ny: number } {
  const qx = Math.abs(px) - (hw - r);
  const qy = Math.abs(py) - (hh - r);
  const sx = Math.sign(px) || 1;
  const sy = Math.sign(py) || 1;
  if (qx > 0 && qy > 0) {
    const len = Math.hypot(qx, qy);
    return { d: len - r, nx: (qx / len) * sx, ny: (qy / len) * sy };
  }
  if (qx > qy) return { d: qx - r, nx: sx, ny: 0 };
  return { d: qy - r, nx: 0, ny: sy };
}

const cache = new Map<string, GlassMaps>();

export function glassMaps(
  width: number,
  height: number,
  radius: number,
  p: GlassParams,
): GlassMaps | null {
  const w = Math.max(1, Math.round(width));
  const h = Math.max(1, Math.round(height));
  const r = Math.min(radius, w / 2, h / 2);
  const key = `${w}x${h}r${r.toFixed(1)}:${JSON.stringify(p)}`;
  const hit = cache.get(key);
  if (hit) return hit;
  if (typeof document === "undefined") return null;

  const bezel = Math.max(1, Math.min(p.depth, Math.min(w, h) / 2 - 0.5));
  const maxOffset = bezel * p.refraction * 0.5;

  const disp = document.createElement("canvas");
  disp.width = w;
  disp.height = h;
  const spec = document.createElement("canvas");
  spec.width = w;
  spec.height = h;
  const dctx = disp.getContext("2d");
  const sctx = spec.getContext("2d");
  if (!dctx || !sctx) return null;
  const dimg = dctx.createImageData(w, h);
  const simg = sctx.createImageData(w, h);

  // Light direction in screen space; Figma measures the angle so that -45
  // lights the top-left edge.
  const la = (p.lightAngle * Math.PI) / 180;
  const lx = -Math.cos(la);
  const ly = Math.sin(la);
  const sharp = 10 - 8.5 * p.splay;
  const hw = w / 2;
  const hh = h / 2;

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4;
      const { d, nx, ny } = roundedRect(x + 0.5 - hw, y + 0.5 - hh, hw, hh, r);
      const inside = -d;
      let dx = 0;
      let dy = 0;
      let a = 0;
      if (inside >= 0 && inside < bezel) {
        const t = inside / bezel;
        const m = (bend(t) / BEND_MAX) * maxOffset;
        // Sample toward the centre: the rim shows what lies further in,
        // stretched outward, like looking through a lens edge.
        dx = -nx * m;
        dy = -ny * m;

        // Specular: bright where the bevel faces the light, a fainter echo
        // on the opposite side, concentrated at the rim.
        const facing = nx * lx + ny * ly;
        const lit =
          Math.max(0, facing) ** sharp + 0.45 * Math.max(0, -facing) ** sharp;
        const rim = Math.exp(-inside / Math.max(1.2, bezel * 0.18));
        a = Math.min(1, lit * rim * p.lightIntensity * 1.15);
      }
      // A hairline all the way round, so the pane has an edge in the dark.
      if (inside >= 0 && inside < 1.25) {
        a = Math.max(a, 0.16 * p.lightIntensity);
      }
      dimg.data[i] = 128 + (maxOffset > 0 ? (dx / maxOffset) * 127 : 0);
      dimg.data[i + 1] = 128 + (maxOffset > 0 ? (dy / maxOffset) * 127 : 0);
      dimg.data[i + 2] = 128;
      dimg.data[i + 3] = 255;
      simg.data[i] = 255;
      simg.data[i + 1] = 255;
      simg.data[i + 2] = 255;
      simg.data[i + 3] = Math.round(a * 255);
    }
  }
  dctx.putImageData(dimg, 0, 0);
  sctx.putImageData(simg, 0, 0);

  const maps: GlassMaps = {
    displacement: disp.toDataURL("image/png"),
    specular: spec.toDataURL("image/png"),
    scale: maxOffset * 2,
  };
  if (cache.size > 64) cache.clear();
  cache.set(key, maps);
  return maps;
}
