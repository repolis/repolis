import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { useForm } from "react-hook-form";

import init, { load_city_data } from "./wasm/engine";

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

  const [wasmReady, setWasmReady] = useState(false);

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
      if (data.cityData) {
        load_city_data(data.cityData);
      }
    },
  });

  useEffect(() => {
    async function loadWasm() {
      try {
        await init();
        setWasmReady(true);
      } catch (e: any) {
        if (e.message?.includes("Using exceptions for control flow")) {
          setWasmReady(true);
        } else if (e.message?.includes("already initialized")) {
          setWasmReady(true);
        } else {
          console.error("Wasm engine failed to boot:", e);
        }
      }
    }
    loadWasm();
  }, []);

  const onSubmit = (data: RepoForm) => {
    analyzeMutation.mutate(data.repoUrl);
  };

  return (
    <div className="w-full space-y-1">
      <form
        onSubmit={handleSubmit(onSubmit)}
        className="mx-auto flex w-150 flex-col gap-2 p-2"
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

      <div className="mx-auto w-4/5 px-10">
        <div className="relative aspect-video w-full bg-black">
          {!wasmReady && (
            <div className="absolute p-4 text-white">Booting Wasm...</div>
          )}
          <canvas
            id="bevy-canvas"
            className="h-full w-full"
            onContextMenu={(e) => e.preventDefault()}
          ></canvas>
        </div>
      </div>
    </div>
  );
}

export default App;
