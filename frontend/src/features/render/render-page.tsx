import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useParams } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";

import { reveal, spring, stagger } from "@/shared/lib/motion";
import { sky, skyScenes } from "@/shared/sky/sky";
import { Logo, type Signal } from "@/shared/ui/brand";
import { IconButton } from "@/shared/ui/glass";
import { IconCompass } from "@/shared/ui/icons";
import init, {
  camera_state,
  clear_selection,
  focus_symbol,
  load_city_data,
  reset_camera,
  set_camera_state,
  set_color_mode,
  set_filter,
  set_path_anchor,
  set_timeline,
  show_path_to,
} from "@/wasm/engine";

import { DistrictLabels } from "./district-labels";
import { InspectorPanel } from "./inspector-panel";
import { Legend } from "./legend";
import { AnalysisLoader, type StageProgress } from "./loader";
import { PathBanner } from "./path-banner";
import { Regenerate } from "./regenerate";
import { SearchPalette } from "./search-bar";
import { Stats } from "./stats";
import { StatusLine } from "./status-line";
import { Timeline } from "./timeline";
import { Tooltip } from "./tooltip";
import type { CitySummary, HoverInfo, PathInfo, SelectPayload } from "./types";
import { decodeView, encodeView, isDefaultView } from "./url-state";
import {
  EMPTY_FILTER,
  FilterMenu,
  ModeMenu,
  type FilterState,
} from "./view-controls";

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
  // Presentation only: which pipeline stage the loader should light up, and
  // whether the clouds have parted far enough to bring the HUD in.
  const [stage, setStage] = useState<StageProgress | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [selection, setSelection] = useState<SelectPayload>({ type: "None" });
  const [summary, setSummary] = useState<CitySummary | null>(null);
  const [mode, setMode] = useState("typology");
  const [filter, setFilterState] = useState<FilterState>(EMPTY_FILTER);
  const [anchor, setAnchor] = useState<{ id: string; name: string } | null>(
    null,
  );
  const [path, setPath] = useState<PathInfo | null>(null);

  const startedRef = useRef(false);
  const sourceRef = useRef<EventSource | null>(null);
  // Captured once, before anything can overwrite the fragment.
  const restoreRef = useRef(decodeView(window.location.hash));
  const restoredRef = useRef(false);
  const selectedIdRef = useRef<string | null>(null);

  // Engine -> UI events.
  useEffect(() => {
    const onHover = (e: Event) =>
      setHover((e as CustomEvent<HoverInfo | null>).detail ?? null);
    const onSelect = (e: Event) => {
      const payload = (e as CustomEvent<SelectPayload>).detail ?? {
        type: "None",
      };
      setSelection(payload);
      selectedIdRef.current =
        payload.type === "Building" ? payload.data.id : null;
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

      // Once only: the refinement pass sends a second city, and re-applying
      // would yank the camera back from wherever the user had moved to.
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
          if (v.camera)
            setTimeout(() => set_camera_state(v.camera as string), 60);
        }
      }
    };
    window.addEventListener("repolis:hover", onHover);
    window.addEventListener("repolis:select", onSelect);
    window.addEventListener("repolis:city", onCity);
    window.addEventListener("repolis:path", onPathEvent);
    return () => {
      window.removeEventListener("repolis:hover", onHover);
      window.removeEventListener("repolis:select", onSelect);
      window.removeEventListener("repolis:city", onCity);
      window.removeEventListener("repolis:path", onPathEvent);
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
              if (msg.stage?.name) {
                setStage({
                  name: msg.stage.name,
                  done: msg.stage.done ?? 0,
                  total: msg.stage.total ?? 0,
                });
              }
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
    },
    [repoUrl],
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

  const handlePick = useCallback(
    (nameOrId: string) => focus_symbol(nameOrId),
    [],
  );
  const handleReset = useCallback(() => {
    reset_camera();
    setSelection({ type: "None" });
  }, []);
  const handleClose = useCallback(() => {
    clear_selection();
    setSelection({ type: "None" });
  }, []);

  // replaceState, and only while idle, so orbiting does not bury the back
  // button in history entries.
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
  }, [phase, mode, filter, timelineDay, selection]);

  // Deep link straight into a city: start inside the clouds, not the sky.
  useLayoutEffect(() => {
    if (sky.fog.get() < 0.9) skyScenes.cover();
    void skyScenes.holding();
  }, []);

  // The first city to arrive parts the clouds; the HUD follows them in.
  useEffect(() => {
    if (phase !== "ready" || revealed) return;
    void skyScenes.reveal();
    const id = window.setTimeout(() => setRevealed(true), 900);
    return () => window.clearTimeout(id);
  }, [phase, revealed]);

  // While the city is being dragged the HUD recedes, so nothing competes
  // with the camera; it returns the moment the pointer lifts.
  const dragTimer = useRef<number | null>(null);
  const endDrag = useCallback(() => {
    if (dragTimer.current !== null) window.clearTimeout(dragTimer.current);
    dragTimer.current = null;
    delete document.documentElement.dataset.dragging;
  }, []);
  useEffect(() => {
    window.addEventListener("pointerup", endDrag);
    window.addEventListener("pointercancel", endDrag);
    return () => {
      window.removeEventListener("pointerup", endDrag);
      window.removeEventListener("pointercancel", endDrag);
      endDrag();
    };
  }, [endDrag]);

  const goHome = useCallback(async () => {
    // Close the clouds over the city before leaving. A full load, because the
    // engine binds to its canvas once and cannot be re-attached.
    await Promise.race([
      Promise.all([
        skyScenes.holding(),
        new Promise((r) => setTimeout(r, 700)),
      ]),
      new Promise((r) => setTimeout(r, 1200)),
    ]);
    window.location.assign("/");
  }, []);

  // Amber only while something is actually streaming in. A draft city with
  // no model configured stays unrefined for good, and is still complete.
  const signal: Signal = phase === "error" ? "error" : busy ? "busy" : "live";
  const note = phase === "ready" && busy ? status : null;

  return (
    <div className="h-full w-full">
      <canvas
        id="bevy-canvas"
        className="absolute inset-0 h-full w-full outline-none"
        onContextMenu={(e) => e.preventDefault()}
        onPointerDown={() => {
          dragTimer.current = window.setTimeout(() => {
            document.documentElement.dataset.dragging = "1";
          }, 140);
        }}
      />

      <AnimatePresence>
        {phase !== "ready" && (
          <AnalysisLoader
            key="loader"
            owner={owner}
            repo={repo}
            phase={phase}
            stage={stage}
            status={status}
            error={error}
            onRetry={() => {
              setStage(null);
              void startAnalysis();
            }}
            onHome={() => void goHome()}
          />
        )}
      </AnimatePresence>

      {phase === "ready" && revealed && (
        <>
          <DistrictLabels />
          <motion.header
            className="pointer-events-none absolute inset-x-0 top-0 z-[45]"
            initial="hidden"
            animate="show"
            variants={stagger(0.08)}
          >
            <motion.button
              type="button"
              variants={reveal}
              onClick={() => void goHome()}
              className="hud-dim pointer-events-auto absolute top-[3.75rem] left-[3.75rem] rounded-xl"
              aria-label="Back to the start"
              title="Analyse another repository"
            >
              <Logo />
            </motion.button>

            <motion.div variants={reveal} className="hud-dim">
              <StatusLine
                owner={owner}
                repo={repo}
                signal={signal}
                note={note}
                draft={!refined && !busy}
              />
            </motion.div>

            <motion.div
              variants={reveal}
              className="hud-dim pointer-events-auto absolute top-[3.75rem] right-[3.75rem] flex items-center gap-2.5"
            >
              <SearchPalette summary={summary} onPick={handlePick} />
              <ModeMenu mode={mode} onMode={handleMode} />
              {summary && (
                <FilterMenu
                  summary={summary}
                  filter={filter}
                  onFilter={handleFilter}
                />
              )}
              <Regenerate busy={busy} onRun={handleRegenerate} />
              <IconButton
                size="lg"
                label="Reset view (R)"
                onClick={handleReset}
              >
                <IconCompass />
              </IconButton>
            </motion.div>
          </motion.header>

          {summary && <Stats summary={summary} />}
          <Timeline summary={summary} onChange={handleTimeline} />
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
          <Tooltip hover={selection.type === "None" ? hover : null} />
          <Hints />
        </>
      )}
    </div>
  );
}

/** How to move, shown once after landing and then out of the way. */
function Hints() {
  const [show, setShow] = useState(true);
  useEffect(() => {
    const id = window.setTimeout(() => setShow(false), 7000);
    const hide = () => setShow(false);
    window.addEventListener("wheel", hide, { once: true, passive: true });
    return () => {
      window.clearTimeout(id);
      window.removeEventListener("wheel", hide);
    };
  }, []);
  const items = [
    ["Drag", "orbit"],
    ["Scroll", "zoom"],
    ["WASD", "pan"],
    ["Click", "inspect"],
    ["F", "fly"],
  ];
  return (
    <AnimatePresence>
      {show && (
        <motion.div
          className="text-lift pointer-events-none absolute bottom-[8.25rem] left-1/2 z-40 flex -translate-x-1/2 gap-5 text-[0.9375rem] font-semibold whitespace-nowrap max-[90rem]:bottom-[12.5rem] max-xl:hidden"
          initial={{ opacity: 0, y: 10, filter: "blur(8px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={{ opacity: 0, y: 6, filter: "blur(8px)" }}
          transition={{ ...spring.soft, delay: 0.8 }}
        >
          {items.map(([k, v]) => (
            <span key={k} className="flex items-center gap-1.5">
              <span className="text-white">{k}</span>
              <span className="text-white/60">{v}</span>
            </span>
          ))}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
