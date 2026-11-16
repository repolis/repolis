import { useCallback, useEffect, useRef, useState } from "react";
import { useParams } from "@tanstack/react-router";

import init, {
  camera_state,
  focus_building,
  load_city_data,
  reset_camera,
  set_appearance,
  set_camera_state,
  set_show_links,
} from "@/wasm/engine";

import { computeAppearance, shortestPath } from "./appearance";
import {
  buildCity,
  hoverInfo,
  pathInfo,
  resolveSymbol,
  selection as toSelection,
  type City,
  type EngineTarget,
  type RawCity,
} from "./city";
import { DistrictLabels } from "./district-labels";
import { InspectorPanel } from "./inspector-panel";
import { Legend } from "./legend";
import { PathBanner } from "./path-banner";
import { SearchBar } from "./search-bar";
import { Timeline } from "./timeline";
import { Tooltip } from "./tooltip";
import type { CitySummary, HoverInfo, PathInfo, SelectPayload } from "./types";
import { decodeView, encodeView, isDefaultView } from "./url-state";
import { EMPTY_FILTER, ViewControls, type FilterState } from "./view-controls";

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
  // Separate from `phase`: a regeneration keeps the city on screen.
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
  const [links, setLinks] = useState(false);
  const [anchor, setAnchor] = useState<{ id: string; name: string } | null>(
    null,
  );
  // The traced path: building ids for the highlight, names for the banner.
  const [path, setPath] = useState<{ ids: string[]; info: PathInfo } | null>(
    null,
  );
  const [timelineDay, setTimelineDay] = useState<number | null>(null);

  const startedRef = useRef(false);
  const sourceRef = useRef<EventSource | null>(null);
  // Captured once, before anything can overwrite the fragment.
  const restoreRef = useRef(decodeView(window.location.hash));
  const restoredRef = useRef(false);
  // The engine reports ids; names and metrics come from here.
  const cityRef = useRef<City | null>(null);
  const loadCity = useCallback((raw: RawCity) => {
    cityRef.current = buildCity(raw);
    load_city_data(raw);
  }, []);

  // Engine -> UI events.
  useEffect(() => {
    const onHover = (e: Event) =>
      setHover(
        cityRef.current &&
          hoverInfo(cityRef.current, (e as CustomEvent<EngineTarget>).detail),
      );
    const onSelect = (e: Event) => {
      const detail = (e as CustomEvent<EngineTarget>).detail;
      const payload: SelectPayload = cityRef.current
        ? toSelection(cityRef.current, detail)
        : { type: "None" };
      setSelection(payload);
      // A new selection ends the path view; the anchor stays pinned.
      setPath(null);
    };
    const onCity = () => {
      const s = cityRef.current?.summary;
      if (!s) return;
      setSummary(s);
      setRefined(s.refined);
      setPhase("ready");
      // Colour, filter and timeline carry over a reload; ids may not.
      setAnchor(null);
      setPath(null);

      // Once only: the refinement pass sends a second city, and re-applying
      // would yank the camera back from wherever the user had moved to.
      if (!restoredRef.current) {
        restoredRef.current = true;
        const v = restoreRef.current;
        if (!isDefaultView(v)) {
          setMode(v.mode);
          setFilterState(v.filter);
          setTimelineDay(v.timeline);
          const at = v.selected
            ? cityRef.current?.indexOf.get(v.selected)
            : undefined;
          if (at !== undefined) focus_building(at);
          // After the focus, which moves the camera itself.
          if (v.camera)
            setTimeout(() => set_camera_state(v.camera as string), 60);
        }
      }
    };
    const onMove = (e: MouseEvent) => setCursor({ x: e.clientX, y: e.clientY });

    window.addEventListener("repolis:hover", onHover);
    window.addEventListener("repolis:select", onSelect);
    window.addEventListener("repolis:city", onCity);
    window.addEventListener("mousemove", onMove);
    return () => {
      window.removeEventListener("repolis:hover", onHover);
      window.removeEventListener("repolis:select", onSelect);
      window.removeEventListener("repolis:city", onCity);
      window.removeEventListener("mousemove", onMove);
    };
  }, []);

  const startAnalysis = useCallback(
    async (refresh = "") => {
      sourceRef.current?.close();
      sourceRef.current = null;
      setBusy(true);
      setError(null);
      // Only the first load takes over with the full-screen panel.
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
          loadCity(data.cityData);
          setBusy(false);
          return;
        }
        if (!data.job_id) {
          setError("Server did not start an analysis");
          setPhase("error");
          setBusy(false);
          return;
        }

        // The draft city arrives over SSE too: the deterministic pass lands in
        // seconds and the LLM pass replaces it, so the city is usable long
        // before the model finishes.
        const src = new EventSource(`/api/jobs/${data.job_id}/events`, {
          withCredentials: true,
        });
        sourceRef.current = src;

        src.onmessage = (ev) => {
          const msg = JSON.parse(ev.data);
          switch (msg.type) {
            case "stage": {
              const label =
                STAGE_LABELS[msg.stage?.name] ?? msg.stage?.name ?? "Working";
              setStatus(
                msg.stage?.total > 0
                  ? `${label} ${msg.stage.done}/${msg.stage.total}`
                  : `${label}…`,
              );
              break;
            }
            case "city":
              loadCity(msg.city);
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
    },
    [repoUrl, loadCity],
  );

  useEffect(() => {
    let cancelled = false;
    async function boot() {
      try {
        await init();
      } catch (e: unknown) {
        const m = e instanceof Error ? e.message : "";
        if (
          !m.includes("Using exceptions for control flow") &&
          !m.includes("already initialized")
        ) {
          if (!cancelled) {
            setError(
              "The 3D engine failed to start. Your browser may not support WebGPU or WebGL2.",
            );
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
      setSelection({ type: "None" });
      void startAnalysis(level);
    },
    [startAnalysis],
  );

  const handleLinks = useCallback((on: boolean) => {
    setLinks(on);
    set_show_links(on);
  }, []);

  const handlePath = useCallback(
    (to: string) => {
      const city = cityRef.current;
      if (!city || !anchor) return;
      const query = {
        from_id: anchor.id,
        to_id: to,
        hops: shortestPath(city.adj, anchor.id, to),
      };
      setPath({ ids: query.hops, info: pathInfo(city, query) });
    },
    [anchor],
  );

  const handleAnchor = useCallback(
    (id: string) => {
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
    setAnchor(null);
    setPath(null);
  }, []);

  const handlePick = useCallback((nameOrId: string) => {
    const id = cityRef.current && resolveSymbol(cityRef.current, nameOrId);
    const at = id ? cityRef.current?.indexOf.get(id) : undefined;
    if (at !== undefined) focus_building(at);
  }, []);
  const handleReset = useCallback(() => {
    reset_camera();
    setSelection({ type: "None" });
    setAnchor(null);
    setPath(null);
  }, []);
  const handleClose = useCallback(() => {
    setSelection({ type: "None" });
    setPath(null);
  }, []);

  const selectedId = selection.type === "Building" ? selection.data.id : null;

  // Everything visible is decided here; the engine only applies it.
  useEffect(() => {
    const city = cityRef.current;
    if (phase !== "ready" || !city) return;
    const a = computeAppearance(city, {
      mode,
      filter,
      timeline: timelineDay,
      selectedId,
      path: path?.ids ?? [],
    });
    set_appearance(a.colors, a.states, a.grounds, a.links);
  }, [phase, summary, mode, filter, timelineDay, selectedId, path]);

  // replaceState, and only while idle, so orbiting does not bury the back
  // button in history entries.
  useEffect(() => {
    if (phase !== "ready") return;
    const write = () => {
      const hash = encodeView({
        mode,
        filter,
        selected: selectedId,
        timeline: timelineDay,
        camera: camera_state() || null,
      });
      const next = window.location.pathname + window.location.search + hash;
      if (
        next !==
        window.location.pathname + window.location.search + window.location.hash
      ) {
        window.history.replaceState(null, "", next);
      }
    };
    const id = window.setInterval(write, 700);
    write();
    return () => window.clearInterval(id);
  }, [phase, mode, filter, timelineDay, selectedId]);

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-black">
      <canvas
        id="bevy-canvas"
        className="h-full w-full"
        onContextMenu={(e) => e.preventDefault()}
      />

      {phase === "ready" && (
        <>
          <DistrictLabels districts={summary?.districts ?? []} />
          <div className="pointer-events-none absolute bottom-2.5 left-3 z-30 font-mono text-[11px] text-white/80">
            drag: orbit · scroll: zoom · WASD: pan · click: select · Esc: clear
            · R: reset
          </div>
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
            links={links}
            onMode={setMode}
            onFilter={setFilterState}
            onLinks={handleLinks}
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
            path={path?.info ?? null}
            onClear={handleClearPath}
          />
          <Tooltip
            hover={selection.type === "None" ? hover : null}
            pos={cursor}
          />
          <Timeline summary={summary} onChange={setTimelineDay} />
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
