import { useEffect, useState } from "react";

export interface RepoFacts {
  /** Canonical owner/repo as GitHub spells it (renames are followed). */
  fullName: string;
  stars: number;
  language: string | null;
}

export type Lookup =
  | { state: "idle" }
  | { state: "checking"; slug: string }
  | { state: "found"; slug: string; facts: RepoFacts }
  | { state: "missing"; slug: string }
  /** GitHub could not be asked (offline, rate limited): never block on it. */
  | { state: "unknown"; slug: string };

const cache = new Map<string, Lookup>();

/**
 * Confirms that a public repository exists before offering to build it, so
 * a typo shows up in the field instead of after a trip into the clouds.
 * Debounced, cancelled on every keystroke and cached per session.
 */
export function useRepoLookup(slug: string | null, delay = 450): Lookup {
  const [result, setResult] = useState<Lookup>({ state: "idle" });

  useEffect(() => {
    if (!slug) {
      const id = window.setTimeout(() => setResult({ state: "idle" }), 0);
      return () => window.clearTimeout(id);
    }
    const key = slug.toLowerCase();
    const known = cache.get(key);
    const ctrl = new AbortController();
    const id = window.setTimeout(
      async () => {
        if (known) {
          setResult(known);
          return;
        }
        setResult({ state: "checking", slug });
        let next: Lookup;
        try {
          const res = await fetch(`https://api.github.com/repos/${slug}`, {
            signal: ctrl.signal,
            headers: { Accept: "application/vnd.github+json" },
          });
          if (res.ok) {
            const body = (await res.json()) as {
              full_name?: string;
              stargazers_count?: number;
              language?: string | null;
            };
            next = {
              state: "found",
              slug,
              facts: {
                fullName: body.full_name ?? slug,
                stars: body.stargazers_count ?? 0,
                language: body.language ?? null,
              },
            };
          } else if (res.status === 404) {
            next = { state: "missing", slug };
          } else {
            next = { state: "unknown", slug };
          }
        } catch {
          if (ctrl.signal.aborted) return;
          next = { state: "unknown", slug };
        }
        if (next.state !== "unknown") cache.set(key, next);
        setResult(next);
      },
      known ? 0 : delay,
    );
    return () => {
      window.clearTimeout(id);
      ctrl.abort();
    };
  }, [slug, delay]);

  if (!slug) return { state: "idle" };
  // While the lookup for a new slug is pending, say so rather than showing
  // the answer for what was typed before.
  if (result.state === "idle" || result.slug !== slug) {
    return { state: "checking", slug };
  }
  return result;
}

export function formatStars(n: number): string {
  if (n < 1000) return String(n);
  if (n < 10000) return `${(n / 1000).toFixed(1).replace(/\.0$/, "")}k`;
  return `${Math.round(n / 1000)}k`;
}
