import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "@tanstack/react-router";

import init, {
  load_city_data,
  focus_symbol,
  reset_camera,
  clear_selection,
  set_color_mode,
  set_filter,
  show_path_to,
  set_timeline,
  set_path_anchor,
  camera_state,
  set_camera_state,
} from "@/wasm/engine";

import { InspectorPanel } from "./inspector-panel";
import { Legend } from "./legend";
import { SearchBar } from "./search-bar";
import { Tooltip } from "./tooltip";
import { Timeline } from "./timeline";
import { ViewControls, EMPTY_FILTER, type FilterState } from "./view-controls";
import { PathBanner } from "./path-banner";
import { decodeView, encodeView, isDefaultView } from "./url-state";
import type { CitySummary, HoverInfo, PathInfo, SelectPayload } from "./types";

type Phase = "booting" | "working" | "ready" | "error";

const STAGE_LABELS: Record<string, string> = {
  cloning: "Cloning repository",
  parsing: "Parsing source",
  history: "Reading git history",
  linking: "Linking call graph",
  associating: "Resolving ambiguous functions",
  naming: "Naming districts",
};

export default function RenderPage() {
  const { owner, repo } = useParams({ from: "/city/$owner/$repo" });
  const repoUrl = `https://github.com/${owner}/${repo}`;

  const [phase, setPhase] = useState<Phase>("booting");
  // Separate from `phase`: a regeneration keeps the existing city on screen
  // rather than throwing the user back to a full-screen loading panel.
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("Booting engine…");
  const [error, setError] = useState<string | null>(null);
  const [refined, setRefined] = useState(false);

  const [hover, setHover] = useState<HoverInfo | null>(null);
  const [cursor, setCursor] = useState({ x: 0, y: 0 });
  const [selection, setSelection] = useState<SelectPayload>({ type: "None" });
  const [summary, setSummary] = useState<CitySummary | null>(null);
  const [mode, setMode] = useState("typology");
  const [filter, setFilterState] = useState<FilterState>(EMPTY_FILTER);
  const [anchor, setAnchor] = useState<{ id: string; name: string } | null>(null);
  const [path, setPath] = useState<PathInfo | null>(null);

  const startedRef = useRef(false);
  const sourceRef = useRef<EventSource | null>(null);
  // Captured once, before anything can overwrite the fragment.
  const restoreRef = useRef(decodeView(window.location.hash));
  const restoredRef = useRef(false);
  const selectedIdRef = useRef<string | null>(null);

  // Engine -> UI events.
  useEffect(() => {
    const onHover = (e: Event) => setHover((e as CustomEvent<HoverInfo | null>).detail ?? null);
    const onSelect = (e: Event) => {
      const payload = (e as CustomEvent<SelectPayload>).detail ?? { type: "None" };
      setSelection(payload);
      selectedIdRef.current = payload.type === "Building" ? payload.data.id : null;
    };
    const onPathEvent = (e: Event) =>
      setPath((e as CustomEvent<PathInfo | null>).detail ?? null);
    const onCity = (e: Event) => {
      const s = (e as CustomEvent<CitySummary>).detail;
      setSummary(s);
      setRefined(s.refined);
      setPhase("ready");
      // A reload rebuilds the city, so the engine's view state resets too.
      setMode("typology");
      setFilterState(EMPTY_FILTER);
      setAnchor(null);
      setPath(null);

      // Restore the linked view once, after the first city arrives. The
      // refinement pass sends a second city; re-applying then would yank the
      // camera back from wherever the user had moved to.
      if (!restoredRef.current) {
        restoredRef.current = true;
        const v = restoreRef.current;
        if (!isDefaultView(v)) {
          if (v.mode !== "typology") {
            setMode(v.mode);
            set_color_mode(v.mode);
          }
          if (v.filter) {
            setFilterState(v.filter);
            set_filter(JSON.stringify(v.filter));
          }
          if (v.timeline !== null) set_timeline(String(v.timeline));
          if (v.selected) focus_symbol(v.selected);
          // After focus_symbol, which moves the camera itself.
          if (v.camera) setTimeout(() => set_camera_state(v.camera as string), 60);
        }
      }
    };
    const onMove = (e: MouseEvent) => setCursor({ x: e.clientX, y: e.clientY });

    window.addEventListener("repolis:hover", onHover);
    window.addEventListener("repolis:select", onSelect);
    window.addEventListener("repolis:city", onCity);
    window.addEventListener("repolis:path", onPathEvent);
    window.addEventListener("mousemove", onMove);
    return () => {
      window.removeEventListener("repolis:hover", onHover);
      window.removeEventListener("repolis:select", onSelect);
      window.removeEventListener("repolis:city", onCity);
      window.removeEventListener("repolis:path", onPathEvent);
      window.removeEventListener("mousemove", onMove);
    };
  }, []);

  const startAnalysis = useCallback(async (refresh = "") => {
    sourceRef.current?.close();
    sourceRef.current = null;
    setBusy(true);
    setError(null);
    // A regeneration leaves the current city on screen; only the very first
    // load is allowed to take over with the full-screen panel.
    if (!refresh) setPhase("working");
    setStatus(refresh ? "Regenerating\u2026" : "Contacting server\u2026");
    try {
      const res = await fetch("/api/analyze", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({ repo_url: repoUrl, refresh }),
      });
      const data = await res.json();

      if (!res.ok) {
        setError(data.error ?? "Analysis failed");
        setPhase("error");
        setBusy(false);
        return;
      }

      // A cached city comes straight back; otherwise we follow the job.
      if (data.cityData) {
        load_city_data(data.cityData);
        setBusy(false);
        return;
      }
      if (!data.job_id) {
        setError("Server did not start an analysis");
        setPhase("error");
        setBusy(false);
        return;
      }

      // Progress arrives over SSE, and so does the draft city: the
      // deterministic pass is published in seconds and the LLM pass replaces
      // it when it lands, so the city is usable long before the model
      // finishes.
      const src = new EventSource(`/api/jobs/${data.job_id}/events`, {
        withCredentials: true,
      });
      sourceRef.current = src;

      src.onmessage = (ev) => {
        const msg = JSON.parse(ev.data);
        switch (msg.type) {
          case "stage": {
            const label = STAGE_LABELS[msg.stage?.name] ?? msg.stage?.name ?? "Working";
            setStatus(
              msg.stage?.total > 0 ? `${label} ${msg.stage.done}/${msg.stage.total}` : `${label}…`,
            );
            break;
          }
          case "city":
            load_city_data(msg.city);
            setStatus(msg.refined ? "Done" : "Refining with the model…");
            break;
          case "error":
            setError(msg.error ?? "Analysis failed");
            setPhase("error");
            setBusy(false);
            src.close();
            break;
          case "done":
            setBusy(false);
            src.close();
            break;
        }
      };
      src.onerror = () => {
        setBusy(false);
        src.close();
      };
    } catch {
      setError("Could not reach the server");
      setPhase("error");
      setBusy(false);
    }
  }, [repoUrl]);

  useEffect(() => {
    let cancelled = false;
    async function boot() {
      try {
        await init();
      } catch (e: unknown) {
        const m = e instanceof Error ? e.message : "";
        if (!m.includes("Using exceptions for control flow") && !m.includes("already initialized")) {
          if (!cancelled) {
            setError("The 3D engine failed to start. Your browser may not support WebGPU or WebGL2.");
            setPhase("error");
          }
          return;
        }
      }
      if (!cancelled && !startedRef.current) {
        startedRef.current = true;
        void startAnalysis();
      }
    }
    void boot();
    return () => {
      cancelled = true;
      sourceRef.current?.close();
    };
  }, [startAnalysis]);

  const handleRegenerate = useCallback(
    (level: string) => {
      clear_selection();
      setSelection({ type: "None" });
      void startAnalysis(level);
    },
    [startAnalysis],
  );

  const handleMode = useCallback((m: string) => {
    setMode(m);
    set_color_mode(m);
  }, []);

  const handleFilter = useCallback((f: FilterState) => {
    setFilterState(f);
    set_filter(JSON.stringify(f));
  }, []);

  const handlePath = useCallback((id: string) => show_path_to(id), []);

  const handleAnchor = useCallback(
    (id: string) => {
      set_path_anchor(id);
      setPath(null);
      if (!id) {
        setAnchor(null);
      } else if (selection.type === "Building") {
        setAnchor({ id, name: selection.data.name });
      }
    },
    [selection],
  );

  const handleClearPath = useCallback(() => {
    set_path_anchor("");
    setAnchor(null);
    setPath(null);
  }, []);

  const [timelineDay, setTimelineDay] = useState<number | null>(null);
  const handleTimeline = useCallback((d: number | null) => {
    setTimelineDay(d);
    set_timeline(d === null ? "" : String(d));
  }, []);

  const handlePick = useCallback((nameOrId: string) => focus_symbol(nameOrId), []);
  const handleReset = useCallback(() => {
    reset_camera();
    setSelection({ type: "None" });
  }, []);
  const handleClose = useCallback(() => {
    clear_selection();
    setSelection({ type: "None" });
  }, []);

  // Mirror the view into the fragment. replaceState, and only while the user
  // is idle, so orbiting does not bury the back button in history entries.
  useEffect(() => {
    if (phase !== "ready") return;
    const write = () => {
      const hash = encodeView({
        mode,
        filter,
        selected: selectedIdRef.current,
        timeline: timelineDay,
        camera: camera_state() || null,
      });
      const next = window.location.pathname + window.location.search + hash;
      if (next !== window.location.pathname + window.location.search + window.location.hash) {
        window.history.replaceState(null, "", next);
      }
    };
    const id = window.setInterval(write, 700);
    write();
    return () => window.clearInterval(id);
  }, [phase, mode, filter, timelineDay, selection]);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-black">
      <canvas
        id="bevy-canvas"
        className="h-full w-full"
        onContextMenu={(e) => e.preventDefault()}
      />

      {phase === "ready" && (
        <>
          <SearchBar
            summary={summary}
            busy={busy}
            onPick={handlePick}
            onReset={handleReset}
            onRegenerate={handleRegenerate}
          />
          <ViewControls
            summary={summary}
            mode={mode}
            filter={filter}
            onMode={handleMode}
            onFilter={handleFilter}
          />
          <Legend summary={summary} mode={mode} />
          <InspectorPanel
            selection={selection}
            repoUrl={repoUrl}
            onClose={handleClose}
            onPick={handlePick}
            onAnchor={handleAnchor}
            onPath={handlePath}
            anchorId={anchor?.id ?? null}
          />
          <PathBanner
            anchorName={anchor?.name ?? null}
            path={path}
            onClear={handleClearPath}
          />
          <Tooltip hover={selection.type === "None" ? hover : null} pos={cursor} />
          <Timeline summary={summary} onChange={handleTimeline} />
          {(busy || !refined) && (
            <div className="absolute bottom-16 left-1/2 z-40 -translate-x-1/2 rounded border border-gray-700 bg-gray-900/95 px-3 py-1.5 text-xs text-gray-300">
              {status}
            </div>
          )}
        </>
      )}

      {phase !== "ready" && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/80">
          <div className="w-80 rounded border border-gray-700 bg-gray-900 p-5 text-sm text-gray-200">
            <div className="font-semibold text-white">
              {owner}/{repo}
            </div>
            {phase === "error" ? (
              <div className="mt-2 text-red-400">{error}</div>
            ) : (
              <>
                <div className="mt-2 text-gray-400">{status}</div>
                <div className="mt-3 h-1 w-full overflow-hidden rounded bg-gray-800">
                  <div className="h-full w-1/3 animate-pulse rounded bg-gray-500" />
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
