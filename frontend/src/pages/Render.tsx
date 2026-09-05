import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";

import { renderRoute } from "../main";
import init, { load_city_data } from "../wasm/engine";

interface CityMapResponse {
  status: string;
  cityData?: any;
}

export default function Render() {
  const { _splat } = renderRoute.useParams();
  const fullUrl = `https://${_splat}`;

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
      if (data.cityData && wasmReady) {
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
        if (
          e.message?.includes("Using exceptions for control flow") ||
          e.message?.includes("already initialized")
        ) {
          setWasmReady(true);
        } else {
          console.error("Wasm engine failed to boot:", e);
        }
      }
    }
    loadWasm();
  }, []);

  useEffect(() => {
    if (wasmReady && _splat) {
      analyzeMutation.mutate(fullUrl);
    }
  }, [wasmReady, _splat]);

  return (
    <div className="relative flex h-screen w-screen flex-col">
      <div className="absolute h-full w-full overflow-hidden bg-black">
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
  );
}
