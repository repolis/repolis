/**
 * Recently opened repositories, per browser. A convenience only: every read
 * and write is guarded, because storage can be missing or blocked.
 */
const KEY = "repolis:recent";
const LIMIT = 4;

export function readRecent(): string[] {
  try {
    const raw = window.localStorage.getItem(KEY);
    const list = raw ? (JSON.parse(raw) as unknown) : [];
    return Array.isArray(list)
      ? list.filter((s): s is string => typeof s === "string").slice(0, LIMIT)
      : [];
  } catch {
    return [];
  }
}

export function pushRecent(slug: string): void {
  try {
    const next = [slug, ...readRecent().filter((s) => s !== slug)].slice(
      0,
      LIMIT,
    );
    window.localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    // Storage unavailable: nothing to remember, nothing to break.
  }
}
