export interface RepoParam {
  repo: string;
  commit: string | null;
}

export function splitCommit(param: string): RepoParam {
  const at = param.lastIndexOf("@");
  if (at === -1) return { repo: param, commit: null };
  return { repo: param.slice(0, at), commit: param.slice(at + 1) };
}

export function pinnedPath(
  host: string,
  owner: string,
  repo: string,
  commit: string,
): string {
  return `/city/${host}/${owner}/${repo}@${commit}`;
}
