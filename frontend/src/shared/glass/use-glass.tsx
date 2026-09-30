import {
  useId,
  useLayoutEffect,
  useMemo,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";

import { useCanRefract } from "@/shared/lib/refraction";

import { glassMaps, type GlassParams } from "./maps";

export type GlassRadius = number | "full";

/** The Figma frame is 1920 wide; bevel depths scale with the root size. */
function rootScale(): number {
  if (typeof window === "undefined") return 1;
  const px = parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(px) && px > 0 ? px / 16 : 1;
}

/**
 * Wires an element up as liquid glass: measures it, bakes displacement and
 * specular maps for its exact size, and returns the backdrop style plus the
 * layers to render inside it. Chromium refracts through an SVG filter; other
 * browsers get the same pane with a frosted backdrop instead of the bend.
 */
export function useGlass<T extends HTMLElement>(
  params: GlassParams,
  radius: GlassRadius,
): { attach: (el: T | null) => void; style: CSSProperties; layers: ReactNode } {
  // A callback ref kept in state, so a remounted element is measured again.
  const [el, attach] = useState<T | null>(null);
  const [size, setSize] = useState<{ w: number; h: number } | null>(null);
  const canRefract = useCanRefract();
  const id = `lg${useId().replace(/[^a-zA-Z0-9]/g, "")}`;

  useLayoutEffect(() => {
    if (!el) return;
    let raf = 0;
    const measure = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        // offsetWidth ignores transforms, so a pane that is still scaling in
        // is measured at its real size.
        const w = Math.round(el.offsetWidth);
        const h = Math.round(el.offsetHeight);
        setSize((s) => (s && s.w === w && s.h === h ? s : { w, h }));
      });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [el]);

  const k = rootScale();
  const scaled = useMemo<GlassParams>(
    () => ({ ...params, depth: params.depth * k, frost: params.frost * k }),
    [params, k],
  );

  const maps = useMemo(() => {
    if (!size || size.w < 2 || size.h < 2) return null;
    const r = radius === "full" ? size.h / 2 : radius * k;
    return glassMaps(size.w, size.h, r, scaled);
  }, [size, radius, scaled, k]);

  const refract = canRefract && maps !== null;
  const blur = scaled.frost / 2;
  const fallbackBlur = Math.max(blur, 10 * k);

  const style: CSSProperties = refract
    ? {
        backdropFilter: `url(#${id})`,
        WebkitBackdropFilter: `blur(${fallbackBlur}px)`,
      }
    : {
        backdropFilter: `blur(${fallbackBlur}px) saturate(140%)`,
        WebkitBackdropFilter: `blur(${fallbackBlur}px) saturate(140%)`,
      };

  const spread = 0.06 * params.dispersion;
  const layers = (
    <>
      {refract && size && maps && (
        <svg
          aria-hidden
          focusable="false"
          width="0"
          height="0"
          className="pointer-events-none absolute"
        >
          <filter
            id={id}
            x="0"
            y="0"
            width={size.w}
            height={size.h}
            filterUnits="userSpaceOnUse"
            primitiveUnits="userSpaceOnUse"
            colorInterpolationFilters="sRGB"
          >
            <feGaussianBlur
              in="SourceGraphic"
              stdDeviation={blur}
              edgeMode="duplicate"
              result="frost"
            />
            <feImage
              href={maps.displacement}
              x="0"
              y="0"
              width={size.w}
              height={size.h}
              preserveAspectRatio="none"
              result="map"
            />
            {spread > 0 ? (
              <>
                <feDisplacementMap
                  in="frost"
                  in2="map"
                  scale={maps.scale * (1 + spread)}
                  xChannelSelector="R"
                  yChannelSelector="G"
                  result="dr"
                />
                <feColorMatrix
                  in="dr"
                  type="matrix"
                  values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0"
                  result="r"
                />
                <feDisplacementMap
                  in="frost"
                  in2="map"
                  scale={maps.scale}
                  xChannelSelector="R"
                  yChannelSelector="G"
                  result="dg"
                />
                <feColorMatrix
                  in="dg"
                  type="matrix"
                  values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0"
                  result="g"
                />
                <feDisplacementMap
                  in="frost"
                  in2="map"
                  scale={maps.scale * (1 - spread)}
                  xChannelSelector="R"
                  yChannelSelector="G"
                  result="db"
                />
                <feColorMatrix
                  in="db"
                  type="matrix"
                  values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0"
                  result="b"
                />
                <feBlend in="r" in2="g" mode="screen" result="rg" />
                <feBlend in="rg" in2="b" mode="screen" />
              </>
            ) : (
              <feDisplacementMap
                in="frost"
                in2="map"
                scale={maps.scale}
                xChannelSelector="R"
                yChannelSelector="G"
              />
            )}
          </filter>
        </svg>
      )}
      {maps && (
        <span
          aria-hidden
          className="pointer-events-none absolute inset-0 rounded-[inherit]"
          style={{
            backgroundImage: `url(${maps.specular})`,
            backgroundSize: "100% 100%",
            mixBlendMode: "plus-lighter",
          }}
        />
      )}
    </>
  );

  return { attach, style, layers };
}
