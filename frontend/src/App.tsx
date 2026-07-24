import { useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";

import init, { say_hello_from_rust } from "./wasm/engine";

interface RepoForm {
  repoUrl: string;
}

interface CityMapResponse {
  status: string;
  cityData?: any;
}

function App() {
  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<RepoForm>();

  const analyzeMutation = useMutation({
    mutationFn: async (url: string): Promise<CityMapResponse> => {
      const res = await fetch("/api/analyze", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ repo_url: url }),
      });

      if (!res.ok) {
        throw new Error("Failed to fetch repository data");
      }
      return res.json();
    },
    onSuccess: (data) => {
      console.log("Successfully generated city map:", data);
    },
  });

  useEffect(() => {
    async function loadWasm() {
      await init();
      say_hello_from_rust();
    }
    loadWasm();
  }, []);

  const onSubmit = (data: RepoForm) => {
    analyzeMutation.mutate(data.repoUrl);
  };

  return (
    <form
      onSubmit={handleSubmit(onSubmit)}
      className="flex w-150 flex-col gap-2 p-2"
    >
      <div className="flex gap-2">
        <input
          type="url"
          placeholder="https://github.com/username/repo"
          {...register("repoUrl", {
            required: "A repository URL is required",
            pattern: {
              value: /^https?:\/\/(www\.)?(github|gitlab)\.com\/.+\/.+/,
              message: "Must be a valid GitHub or GitLab URL",
            },
          })}
          className="flex-1 rounded-md border-2 border-gray-200 px-4 py-2 transition-colors outline-none focus:border-blue-500"
          disabled={analyzeMutation.isPending}
        />
        <button
          type="submit"
          disabled={analyzeMutation.isPending}
          className="cursor-pointer rounded-md bg-blue-600 px-6 py-2 font-medium text-white transition-colors hover:bg-blue-700 disabled:bg-blue-300"
        >
          {analyzeMutation.isPending ? "Parsing..." : "Generate"}
        </button>
      </div>

      {errors.repoUrl && (
        <span className="text-sm font-medium text-red-500">
          {errors.repoUrl.message}
        </span>
      )}
      {analyzeMutation.isError && (
        <span className="text-sm font-medium text-red-500">
          {analyzeMutation.error.message}
        </span>
      )}
      {analyzeMutation.isSuccess && (
        <span className="text-sm font-medium text-green-600">
          City data received! Check the console.
        </span>
      )}
    </form>
  );
}

export default App;
