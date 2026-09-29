import { useNavigate } from "@tanstack/react-router";
import { useForm } from "react-hook-form";

interface RepoForm {
  repoUrl: string;
}
export default function HomePage() {
  const navigate = useNavigate();
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<RepoForm>();

  const onSubmit = (data: RepoForm) => {
    const cleanPath = data.repoUrl.replace(
      /^https?:\/\/(www\.)?github\.com\//,
      "",
    );
    const parts = cleanPath.split("/");
    if (parts.length >= 2) {
      navigate({
        to: "/city/$owner/$repo",
        params: {
          owner: parts[0],
          repo: parts[1],
        },
      });
    }
  };

  return (
    <div className="bg-[#202020] text-white">
      <div className="flex h-screen w-full items-center justify-center">
        <form
          onSubmit={handleSubmit(onSubmit)}
          className="mx-auto flex w-150 flex-col gap-2 p-2"
        >
          <h1 className="text-center text-4xl font-medium mb-5">Analyze your repo</h1>

          <div className="flex gap-2">
            <input
              type="url"
              placeholder="https://github.com/username/repo"
              {...register("repoUrl", {
                required: "A repository URL is required",
                pattern: {
                  value: /^https?:\/\/(www\.)?github\.com\/.+\/.+/,
                  message: "Must be a valid GitHub URL",
                },
              })}
              className="flex-1 rounded-md border-2 text-white border-gray-200 px-4 py-2 transition-colors outline-none focus:border-blue-500"
            />
            <button
              type="submit"
              className="cursor-pointer rounded-md bg-blue-600 px-6 py-2 font-medium text-white transition-colors hover:bg-blue-700"
            >
              Generate
            </button>
          </div>

          {errors.repoUrl && (
            <span className="text-sm font-medium text-red-500">
              {errors.repoUrl.message}
            </span>
          )}
        </form>
      </div>
    </div>
  );
}
