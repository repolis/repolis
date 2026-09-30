/** 81240 -> "81,240", the way the Figma HUD prints counts. */
export function formatCount(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}

/** Accepts "owner/repo", "github.com/owner/repo" or a full URL. */
export function parseRepo(
  input: string,
): { owner: string; repo: string } | null {
  const cleaned = input
    .trim()
    .replace(/^git@github\.com:/, "")
    .replace(/^https?:\/\//, "")
    .replace(/^(www\.)?github\.com\//, "")
    .replace(/\.git$/, "")
    .replace(/\/+$/, "");
  const [owner, repo] = cleaned.split("/");
  const ok = /^[A-Za-z0-9-_.]+$/;
  if (!owner || !repo || !ok.test(owner) || !ok.test(repo)) return null;
  return { owner, repo };
}

export function formatAge(days: number): string {
  if (days <= 0) return "unknown";
  if (days < 1) return "today";
  if (days < 60) return `${days}d ago`;
  if (days < 730) return `${Math.round(days / 30)}mo ago`;
  return `${(days / 365).toFixed(1)}y ago`;
}
