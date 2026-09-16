import { useEffect, useState, useRef } from "react";
import { useMutation } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";

import init, { load_city_data } from "@/wasm/engine";
import { InspectorPanel, type HoverData } from "./inspector-panel";

interface CityMapResponse {
  status: string;
  cityData?: any;
}

export default function RenderPage() {
  const { owner, repo } = useParams({ from: "/city/$owner/$repo" });
  const fullUrl = `https://github.com/${owner}/${repo}`;

  const [wasmReady, setWasmReady] = useState(false);
  const [hoverData, setHoverData] = useState<HoverData | null>(null);

  useEffect(() => {
    const handleHover = (e: Event) => {
      const customEvent = e as CustomEvent<HoverData>;
      if (customEvent.detail) {
        setHoverData(customEvent.detail);
      }
    };

    window.addEventListener("repolis:hover", handleHover);
    return () => {
      window.removeEventListener("repolis:hover", handleHover);
    };
  }, []);

  const analyzeMutation = useMutation({
    mutationFn: async (url: string): Promise<CityMapResponse> => {
      const res = await fetch("/api/analyze", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ repo_url: url, force: false }),
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

  const hasRequested = useRef(false);

  useEffect(() => {
    if (wasmReady && owner && repo && !hasRequested.current) {
      hasRequested.current = true;
      analyzeMutation.mutate(fullUrl);
    }
  }, [wasmReady, owner, repo, analyzeMutation, fullUrl]);

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
        <InspectorPanel hover={hoverData} />
      </div>
    </div>
  );
}
