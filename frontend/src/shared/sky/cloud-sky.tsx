import { useLayoutEffect, useRef } from "react";

import { isSkyVisible, sky, skyClock } from "./sky";

/*
 * Procedural sky, adapted from the 21st.dev "Cloud Shader" (Aceternity).
 *
 * Added for repolis:
 *   - a full-screen billow blanket (u_fog) the city is built inside of
 *   - distant moonlit wisps and low mist over the landing photograph (u_far)
 *   - parting (u_part): a ragged opening from the centre, pushed through
 *   - a dive zoom (u_zoom) for the fall through the cloud bank
 *   - pointer parallax, nearer clouds moving more
 *   - premultiplied alpha, so the city canvas shows through as they part
 */

const VERT = `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = a_pos * 0.5 + 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
`;

const FRAG = `
precision highp float;
varying vec2 v_uv;

uniform vec2 u_res;
uniform float u_time;
uniform float u_sky;
uniform float u_far;
uniform float u_clouds;
uniform float u_fog;
uniform float u_part;
uniform float u_zoom;
uniform vec2 u_mouse;
uniform vec3 u_cloud;
uniform vec3 u_skyTop;
uniform vec3 u_skyBottom;

const mat2 R = mat2(0.80, 0.60, -0.60, 0.80);

float hash(vec2 p) { return fract(sin(dot(p, vec2(41.31, 289.17))) * 26737.367); }

// Well-mixed hash for sparse points (the sin hash shows grid artefacts).
float hash21(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}

float vnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float fbm(vec2 p) {
  float s = 0.0, a = 0.5;
  for (int i = 0; i < 4; i++) { s += a * vnoise(p); p = R * p * 2.03 + 19.19; a *= 0.5; }
  return s;
}

float billow(vec2 p) {
  float s = 0.0, a = 0.5;
  for (int i = 0; i < 5; i++) { s += a * (1.0 - abs(2.0 * vnoise(p) - 1.0)); p = R * p * 2.11 + 13.37; a *= 0.5; }
  return s;
}

float fbm3(vec2 p) {
  float s = 0.0, a = 0.5;
  for (int i = 0; i < 3; i++) { s += a * vnoise(p); p = R * p * 2.03 + 19.19; a *= 0.5; }
  return s;
}

float billow4(vec2 p) {
  float s = 0.0, a = 0.5;
  for (int i = 0; i < 4; i++) { s += a * (1.0 - abs(2.0 * vnoise(p) - 1.0)); p = R * p * 2.11 + 13.37; a *= 0.5; }
  return s / 0.9375;
}

float cloudDensity(vec2 p, vec2 c, vec2 r, float seed, float t) {
  vec2 q = p - c;
  float ry = q.y > 0.0 ? r.y : r.y * 0.42;
  float env = 1.0 - length(vec2(q.x / r.x, q.y / ry));
  if (env < -0.35) return 0.0;
  vec2 dp = q * (2.4 / r.x) + seed;
  dp += 0.6 * vec2(fbm(dp * 1.4 + t * 0.04), fbm(dp * 1.4 + 7.7 - t * 0.03));
  return env + (billow(dp * 1.6) - 0.62) * 0.62;
}

// Premultiplied "over": src on top of dst.
vec4 over(vec4 src, vec4 dst) { return src + dst * (1.0 - src.a); }

vec4 shadeCloud(vec4 acc, vec3 sky, vec2 p, vec2 c, vec2 r, float seed, float t, float dist) {
  float d = cloudDensity(p, c, r, seed, t);
  if (d < 0.02) return acc;
  float dUp = cloudDensity(p + vec2(0.0, r.y * 0.55), c, r, seed, t);
  float occl = clamp((dUp - d) * 1.1 + d * 0.55, 0.0, 1.0);
  vec3 lit = u_cloud * 1.04;
  vec3 shadow = mix(u_cloud * 0.42, sky, 0.5);
  vec3 col = mix(lit, shadow, occl * 0.8);
  float a = smoothstep(0.02, 0.38, d);
  float rim = smoothstep(0.02, 0.14, d) * (1.0 - smoothstep(0.14, 0.40, d));
  col += rim * 0.10;
  col = mix(col, sky, dist * 0.35);
  a *= mix(1.0, 0.8, dist);
  return over(vec4(col * a, a), acc);
}

vec4 cloudPass(vec4 acc, vec3 sky, vec2 p, float aspect, float t,
               float spd, float phase, float y, vec2 r, float seed, float dist) {
  p += u_mouse * 0.035 * (1.0 - dist);
  float cx = mix(-r.x - 0.25, aspect + r.x + 0.25, fract(t * spd + phase));
  float cy = y + sin(t * 0.05 + phase * 6.2831) * 0.012;
  return shadeCloud(acc, sky, p, vec2(cx, cy), r, seed, t, dist);
}

void main() {
  float aspect = u_res.x / u_res.y;
  vec2 uv = v_uv;
  vec2 center = vec2(0.5 * aspect, 0.5);
  vec2 p = vec2(uv.x * aspect, uv.y);

  // Dive: everything scales toward the viewer around the centre.
  p = center + (p - center) / (1.0 + u_zoom * 0.9);

  float t = u_time;
  vec3 sky = mix(u_skyBottom, u_skyTop, smoothstep(0.0, 1.0, uv.y));
  sky = mix(sky, u_skyBottom * 1.04, smoothstep(0.38, 0.0, uv.y) * 0.5);
  // A cool moon glow high on the right, and a scatter of faint stars.
  vec2 moonPos = vec2(aspect * 0.78, 0.9);
  float moonDist = length(vec2(uv.x * aspect, uv.y) - moonPos);
  sky += vec3(0.62, 0.7, 0.9) * exp(-moonDist * moonDist * 9.0) * 0.22;
  vec2 cell = floor(v_uv * u_res / 2.5);
  float star = step(0.9975, hash21(cell)) * smoothstep(0.3, 0.95, uv.y);
  star *= 0.5 + 0.5 * sin(t * 1.3 + hash21(cell + 7.7) * 40.0);
  sky += vec3(star) * 0.45 * u_sky;

  vec4 clouds = vec4(0.0);

  // Far away over the landing photograph: long wisps high in the sky, lit
  // by the moon near it, and a thin mist drifting over the water.
  if (u_far > 0.001) {
    vec2 pm = vec2(uv.x * aspect, uv.y) + u_mouse * 0.012;
    float band = smoothstep(0.5, 0.72, uv.y) * (1.0 - smoothstep(0.93, 1.0, uv.y));
    float w1 = fbm(vec2(pm.x * 1.1 - t * 0.010, pm.y * 6.0 + 3.0));
    float w2 = fbm(vec2(pm.x * 2.3 - t * 0.016, pm.y * 11.0 - 1.7));
    float wisp = smoothstep(0.5, 0.85, w1 * 0.7 + w2 * 0.45) * band;
    float md = length(vec2(uv.x * aspect, uv.y) - vec2(aspect * 0.85, 0.87));
    float glow = exp(-md * md * 5.0);
    vec3 wc = mix(vec3(0.20, 0.25, 0.36), vec3(0.62, 0.72, 0.92), glow);
    float wa = wisp * (0.22 + 0.4 * glow) * u_far;

    float mistBand = smoothstep(0.18, 0.28, uv.y) * (1.0 - smoothstep(0.34, 0.44, uv.y));
    float m = fbm(vec2(pm.x * 1.6 + t * 0.012, pm.y * 7.0));
    float mist = smoothstep(0.45, 0.8, m) * mistBand * 0.16 * u_far;

    clouds = over(vec4(vec3(0.42, 0.50, 0.62) * mist, mist), clouds);
    clouds = over(vec4(wc * wa, wa), clouds);
  }

  if (u_clouds > 0.001) {
    float cirrusBand = smoothstep(0.55, 0.8, uv.y) * (1.0 - smoothstep(0.92, 1.0, uv.y));
    if (cirrusBand > 0.01) {
      float streak = fbm(vec2(p.x * 1.6 - t * 0.006, p.y * 12.0));
      float wisp = smoothstep(0.52, 0.78, streak) * cirrusBand * 0.35;
      clouds = over(vec4(u_cloud * 0.98 * wisp, wisp), clouds);
    }

    clouds = cloudPass(clouds, sky, p, aspect, t, 0.006, 0.10, 0.84, vec2(0.20, 0.10), 43.7, 1.0);
    clouds = cloudPass(clouds, sky, p, aspect, t, 0.008, 0.62, 0.73, vec2(0.24, 0.12), 71.3, 0.85);
    clouds = cloudPass(clouds, sky, p, aspect, t, 0.011, 0.33, 0.60, vec2(0.34, 0.16), 17.3, 0.55);
    clouds = cloudPass(clouds, sky, p, aspect, t, 0.013, 0.80, 0.47, vec2(0.30, 0.15), 29.9, 0.45);
    clouds = cloudPass(clouds, sky, p, aspect, t, 0.016, 0.05, 0.35, vec2(0.46, 0.20), 91.1, 0.15);
    clouds = cloudPass(clouds, sky, p, aspect, t, 0.020, 0.48, 0.20, vec2(0.56, 0.24), 57.2, 0.0);
    clouds *= u_clouds;
  }

  // Horizon haze, cheap, present whenever there is any fog at all.
  if (u_fog > 0.001) {
    float haze = smoothstep(0.34, 0.0, uv.y) * min(1.0, u_fog * 1.6);
    clouds = over(vec4(u_cloud * 1.02 * haze, haze), clouds);
  }

  // The blanket: two decks of billows, the near one larger and faster, so
  // the cloud bank has depth. It gathers at the edges of the view first and
  // closes in toward the centre, as when flying into a bank of cloud.
  if (u_fog > 0.05) {
    float rim = length((uv - 0.5) * vec2(1.25, 1.0));
    float lo = mix(0.7, 0.4, u_fog) - rim * 0.3 * (1.0 - u_fog);
    vec3 shadowCol = mix(u_cloud * 0.4, u_skyTop, 0.45);
    vec3 litCol = u_cloud * 1.02;
    float grow = smoothstep(0.1, 0.55, u_fog + rim * 0.35 * (1.0 - u_fog));

    // Far deck: smaller cells, drifting slowly, lit from above.
    vec2 q = p * 2.3 + vec2(t * 0.018, -t * 0.004) + u_mouse * 0.04;
    q += 0.5 * vec2(fbm3(q * 0.7 + t * 0.03), fbm3(q * 0.7 - t * 0.02 + 3.1));
    float b = billow4(q);
    float a = smoothstep(lo - 0.1, lo + 0.12, b) * grow;
    float lit = smoothstep(0.45, 0.95, b) * 0.75 + uv.y * 0.35;
    vec3 col = mix(shadowCol, litCol, clamp(lit, 0.0, 1.0));
    clouds = over(vec4(col * a, a), clouds);

    // Near deck: big soft masses sweeping past the lens.
    vec2 n = p * 1.15 + vec2(t * 0.045, t * 0.004) + u_mouse * 0.1 + 11.3;
    n += 0.6 * vec2(fbm3(n * 0.8 - t * 0.03), fbm3(n * 0.8 + t * 0.025 + 5.7));
    float bn = billow4(n);
    float aN = smoothstep(lo + 0.05, lo + 0.25, bn) * smoothstep(0.3, 0.9, u_fog + rim * 0.3 * (1.0 - u_fog)) * 0.72;
    float litN = smoothstep(0.5, 1.0, bn) * 0.8 + uv.y * 0.3;
    vec3 colN = mix(shadowCol * 0.96, litCol, clamp(litN, 0.0, 1.0));
    clouds = over(vec4(colN * aN, aN), clouds);
  }

  vec4 skyLayer = vec4(sky * u_sky, u_sky);
  vec4 color = over(clouds, skyLayer);

  // Reveal: an opening grows from the centre, its edge shaped by the cloud
  // texture, while the camera pushes through (u_zoom). It cuts through the
  // painted sky as well, so the city shows through it directly. Symmetric
  // and continuous: no line where two halves meet.
  if (u_part > 0.0) {
    vec2 q = (uv - 0.5) * vec2(aspect, 1.0);
    float dist = length(q) / length(vec2(aspect, 1.0) * 0.5);
    float ragged = (fbm(p * 2.4 + vec2(t * 0.04, -t * 0.03)) - 0.47) * 0.5;
    float edge = u_part * 1.45 - 0.15;
    float keep = smoothstep(edge - 0.3, edge + 0.05, dist + ragged);
    keep *= 1.0 - smoothstep(0.7, 1.0, u_part);
    color *= mix(1.0, keep, smoothstep(0.0, 0.06, u_part));
  }

  gl_FragColor = color;
}
`;

function hex(color: string): [number, number, number] {
  const h = color.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16) / 255,
    parseInt(h.slice(2, 4), 16) / 255,
    parseInt(h.slice(4, 6), 16) / 255,
  ];
}

function compile(gl: WebGLRenderingContext, type: number, src: string) {
  const s = gl.createShader(type);
  if (!s) return null;
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
    console.warn("[sky] shader:", gl.getShaderInfoLog(s));
    gl.deleteShader(s);
    return null;
  }
  return s;
}

/** Dusk palette: it hands over to the night city's fog colour. */
const PALETTE = {
  cloud: hex("#aab4c5"),
  skyTop: hex("#060a14"),
  skyBottom: hex("#28324a"),
};

/** Clouds are soft; rendering them well below device resolution is
 * invisible, and keeps the shader cheap on integrated GPUs. */
const RENDER_SCALE = 0.5;

export function CloudSky() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // A layout effect, and the first frame drawn before paint: the page never
  // shows an empty sky, which matters when it opens inside the clouds.
  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const gl = canvas.getContext("webgl", {
      alpha: true,
      premultipliedAlpha: true,
      antialias: false,
      powerPreference: "low-power",
    });
    if (!gl) return;

    const vs = compile(gl, gl.VERTEX_SHADER, VERT);
    const fs = compile(gl, gl.FRAGMENT_SHADER, FRAG);
    if (!vs || !fs) return;
    const prog = gl.createProgram();
    if (!prog) return;
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.bindAttribLocation(prog, 0, "a_pos");
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return;
    gl.useProgram(prog);

    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 3, -1, -1, 3]),
      gl.STATIC_DRAW,
    );
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

    const u = (n: string) => gl.getUniformLocation(prog, n);
    const loc = {
      res: u("u_res"),
      time: u("u_time"),
      sky: u("u_sky"),
      far: u("u_far"),
      clouds: u("u_clouds"),
      fog: u("u_fog"),
      part: u("u_part"),
      zoom: u("u_zoom"),
      mouse: u("u_mouse"),
      cloud: u("u_cloud"),
      skyTop: u("u_skyTop"),
      skyBottom: u("u_skyBottom"),
    };
    gl.uniform3f(loc.cloud, ...PALETTE.cloud);
    gl.uniform3f(loc.skyTop, ...PALETTE.skyTop);
    gl.uniform3f(loc.skyBottom, ...PALETTE.skyBottom);

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 1.25) * RENDER_SCALE;
      const w = Math.max(1, Math.floor(canvas.clientWidth * dpr));
      const h = Math.max(1, Math.floor(canvas.clientHeight * dpr));
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
      }
      gl.viewport(0, 0, w, h);
      gl.uniform2f(loc.res, w, h);
    };
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);
    resize();

    // Pointer parallax, eased so the sky never twitches.
    const mouse = { x: 0, y: 0, tx: 0, ty: 0 };
    const onMove = (e: PointerEvent) => {
      mouse.tx = (e.clientX / window.innerWidth) * 2 - 1;
      mouse.ty = -((e.clientY / window.innerHeight) * 2 - 1);
    };
    window.addEventListener("pointermove", onMove, { passive: true });

    const reduce = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    let raf = 0;
    // Continues from the clouds the previous page closed, if any.
    let clock = skyClock.value;
    let last = performance.now();
    let shown = true;

    const frame = (now: number) => {
      raf = requestAnimationFrame(frame);
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;

      // Stop drawing entirely once the city has taken over.
      const visible = isSkyVisible();
      if (visible !== shown) {
        shown = visible;
        canvas.style.visibility = visible ? "visible" : "hidden";
      }
      if (!visible) return;

      if (!reduce) clock += dt * sky.speed.get();
      skyClock.value = clock;
      mouse.x += (mouse.tx - mouse.x) * Math.min(1, dt * 2.5);
      mouse.y += (mouse.ty - mouse.y) * Math.min(1, dt * 2.5);

      gl.uniform1f(loc.time, clock + 40);
      gl.uniform1f(loc.sky, sky.sky.get());
      gl.uniform1f(loc.clouds, sky.clouds.get());
      gl.uniform1f(loc.far, sky.far.get());
      gl.uniform1f(loc.fog, sky.fog.get());
      gl.uniform1f(loc.part, sky.part.get());
      gl.uniform1f(loc.zoom, sky.zoom.get());
      gl.uniform2f(loc.mouse, mouse.x, mouse.y);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };
    frame(performance.now());

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      window.removeEventListener("pointermove", onMove);
      gl.deleteBuffer(buf);
      gl.deleteProgram(prog);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      className="pointer-events-none fixed inset-0 z-20 h-full w-full"
    />
  );
}
