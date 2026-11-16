import { useEffect, useEffectEvent, useRef, useState } from "react";

import init from "@/wasm/engine";

import type { RawCity } from "./city";

const STAGE_LABELS: Record<string, string> = {
  cloning: "Cloning repository",
  parsing: "Parsing source",
  history: "Reading git history",
  linking: "Linking call graph",
  associating: "Resolving ambiguous functions",
  naming: "Naming districts",
};

interface StreamMessage {
  type: "stage" | "city" | "error" | "done";
  stage?: { name: string; done: number; total: number };
  city?: RawCity;
  refined?: boolean;
  error?: string;
}

export function useCityLoader(repoUrl: string, onCity: (raw: RawCity) => void) {
  // Separate from "is a city shown": a regeneration keeps the city on screen.
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("Booting engine…");
  const [error, setError] = useState<string | null>(null);
  const source = useRef<EventSource | null>(null);

  const fail = (message: string) => {
    setError(message);
    setBusy(false);
    source.current?.close();
  };

  async function load(refresh = "") {
    source.current?.close();
    setBusy(true);
    setError(null);
    setStatus(refresh ? "Regenerating…" : "Contacting server…");
    try {
      const res = await fetch("/api/analyze", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ repo_url: repoUrl, refresh }),
      });
      const data = await res.json();
      if (!res.ok) return fail(data.error ?? "Analysis failed");

      if (data.cityData) {
        onCity(data.cityData);
        return setBusy(false);
      }
      if (!data.job_id) return fail("Server did not start an analysis");

      // The deterministic draft arrives over SSE in seconds; the LLM pass replaces it later.
      const src = new EventSource(`/api/jobs/${data.job_id}/events`, {
        withCredentials: true,
      });
      source.current = src;
      src.onerror = () => {
        setBusy(false);
        src.close();
      };
      src.onmessage = (ev) => {
        const msg: StreamMessage = JSON.parse(ev.data);
        if (msg.type === "stage" && msg.stage) {
          const label = STAGE_LABELS[msg.stage.name] ?? msg.stage.name;
          setStatus(
            msg.stage.total > 0
              ? `${label} ${msg.stage.done}/${msg.stage.total}`
              : `${label}…`,
          );
        } else if (msg.type === "city" && msg.city) {
          onCity(msg.city);
          setStatus(msg.refined ? "Done" : "Refining with the model…");
        } else if (msg.type === "error") {
          fail(msg.error ?? "Analysis failed");
        } else if (msg.type === "done") {
          setBusy(false);
          src.close();
        }
      };
    } catch {
      fail("Could not reach the server");
    }
  }
  const start = useEffectEvent(() => void load());

  useEffect(() => {
    let cancelled = false;
    init()
      .catch((e: unknown) => {
        // Bevy's winit loop throws to hand control back to the browser, so
        // that rejection means the engine is running; so does a second init.
        const m = e instanceof Error ? e.message : "";
        if (
          !m.includes("Using exceptions for control flow") &&
          !m.includes("already initialized")
        )
          throw e;
      })
      .then(
        () => !cancelled && start(),
        () =>
          !cancelled &&
          setError(
            "The 3D engine failed to start. Your browser may not support WebGPU or WebGL2.",
          ),
      );
    return () => {
      cancelled = true;
      source.current?.close();
    };
  }, [repoUrl]);

  return { busy, status, error, regenerate: load };
}
