import { useActionState } from "react";
import { useNavigate } from "@tanstack/react-router";

const GITHUB_REPO = /^https?:\/\/(www\.)?github\.com\/([^/]+)\/([^/]+)/;

export default function HomePage() {
  const navigate = useNavigate();

  const [state, submit] = useActionState(
    (_: unknown, form: FormData) => {
      const repoUrl = String(form.get("repoUrl") ?? "").trim();
      const match = GITHUB_REPO.exec(repoUrl);
      if (!match) return { error: "Must be a valid GitHub URL", repoUrl };
      navigate({
        to: "/city/$owner/$repo",
        params: { owner: match[2], repo: match[3] },
      });
      return { error: null, repoUrl };
    },
    { error: null as string | null, repoUrl: "" },
  );

  return (
    <div className="bg-[#202020] text-white">
      <div className="flex h-screen w-full items-center justify-center">
        <form action={submit} className="mx-auto flex w-150 flex-col gap-2 p-2">
          <h1 className="mb-5 text-center text-4xl font-medium">
            Analyze your repo
          </h1>

          <div className="flex gap-2">
            <input
              name="repoUrl"
              type="url"
              required
              defaultValue={state.repoUrl}
              placeholder="https://github.com/username/repo"
              className="flex-1 rounded-md border-2 border-gray-200 px-4 py-2 text-white transition-colors outline-none focus:border-blue-500"
            />
            <button
              type="submit"
              className="cursor-pointer rounded-md bg-blue-600 px-6 py-2 font-medium text-white transition-colors hover:bg-blue-700"
            >
              Generate
            </button>
          </div>

          {state.error && (
            <span className="text-sm font-medium text-red-500">
              {state.error}
            </span>
          )}
        </form>
      </div>
    </div>
  );
}
