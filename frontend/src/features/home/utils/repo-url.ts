import { ALLOWED_HOSTS } from "@/shared/constants";

const REPO_PATH = /^\/([\w.-]+)\/([\w.-]+?)(?:\.git)?\/?$/;

export interface RepoRef {
  host: string;
  owner: string;
  repo: string;
}

export function parseRepoUrl(raw: string): RepoRef | { error: string } {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return { error: "Not a valid URL" };
  }
  if (url.protocol !== "https:")
    return { error: "Only https:// repository URLs are supported" };

  const host = url.hostname.toLowerCase().replace(/^www\./, "");
  if (!ALLOWED_HOSTS.includes(host))
    return { error: `Supported hosts: ${ALLOWED_HOSTS.join(", ")}` };

  const match = REPO_PATH.exec(url.pathname);
  if (!match || url.pathname.includes(".."))
    return { error: `Expected https://${host}/<owner>/<repo>` };
  return { host, owner: match[1], repo: match[2] };
}
