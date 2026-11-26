import { useEffect, useRef, useState } from "react";
import { useParams } from "@tanstack/react-router";

import {
  focus_building,
  load_city_data,
  reset_camera,
  set_appearance,
  set_camera_state,
  set_show_links,
} from "@/wasm/engine";

import { PathBanner } from "./components/path-banner";
import { SearchBar } from "./components/search-bar";
import { Tooltip } from "./components/tooltip";
import { useCityLoader } from "./hooks/use-city-loader";
import { useEngineEvents } from "./hooks/use-engine-events";
import { useUrlSync } from "./hooks/use-url-sync";
import type {
  BuildingInfo,
  CitySummary,
  HoverInfo,
  PathInfo,
  SelectPayload,
} from "./types";
import { computeAppearance } from "./utils/appearance";
import {
  buildCity,
  hoverInfo,
  pathInfo,
  resolveSymbol,
  selection as toSelection,
  type City,
  type RawCity,
} from "./utils/city";
import { isVisible } from "./utils/filter";
import { closedLoop, shortestPath } from "./utils/graph";
import { pinnedPath, splitCommit } from "./utils/permalink";
import { decodeView } from "./utils/url-state";
import { CycleList } from "./widgets/cycle-list";
import { DistrictLabels } from "./widgets/district-labels";
import { InspectorPanel } from "./widgets/inspector/inspector-panel";
import { Legend } from "./widgets/legend";
import { Timeline } from "./widgets/timeline";
import { TopTable } from "./widgets/top-table";
import { ViewControls } from "./widgets/view-controls";

const NOTHING: SelectPayload = { type: "None" };

interface Cycle {
  id: number;
  loops: string[][];
  loop: string[];
}

export default function RenderPage() {
  const params = useParams({ from: "/city/$host/$owner/$repo" });
  const { host, owner } = params;
  const { repo, commit: pinned } = splitCommit(params.repo);
  const repoUrl = `https://${host}/${owner}/${repo}`;

  const [initial] = useState(() => decodeView(window.location.hash));
  const [mode, setMode] = useState(initial.mode);
  const [filter, setFilter] = useState(initial.filter);
  const [timelineDay, setTimelineDay] = useState(initial.timeline);
  const [links, setLinks] = useState(false);
  const [table, setTable] = useState(false);
  const [cycle, setCycle] = useState<Cycle | null>(null);

  const [summary, setSummary] = useState<CitySummary | null>(null);
  const [buildings, setBuildings] = useState<BuildingInfo[]>([]);
  const [hover, setHover] = useState<HoverInfo | null>(null);
  const [selection, setSelection] = useState<SelectPayload>(NOTHING);
  const [anchor, setAnchor] = useState<{ id: string; name: string } | null>(
    null,
  );
  const [path, setPath] = useState<{ ids: string[]; info: PathInfo } | null>(
    null,
  );

  const city = useRef<City | null>(null);
  const restored = useRef(false);
  const focus = (id: string) => {
    const at = city.current?.indexOf.get(id);
    if (at !== undefined) focus_building(at);
  };

  const loader = useCityLoader(repoUrl, pinned, (raw: RawCity) => {
    city.current = buildCity(raw);
    load_city_data(raw);
  });

  useEffect(() => {
    if (!loader.commit) return;
    const path = pinnedPath(host, owner, repo, loader.commit);
    if (path === window.location.pathname) return;
    const rest = window.location.search + window.location.hash;
    window.history.replaceState(null, "", path + rest);
  }, [host, owner, repo, loader.commit]);

  useEngineEvents({
    hover: (t) => setHover(city.current && hoverInfo(city.current, t)),
    select: (t) => {
      setSelection(city.current ? toSelection(city.current, t) : NOTHING);
      setPath(null);
    },
    city: () => {
      if (!city.current) return;
      setSummary(city.current.summary);
      setBuildings(city.current.list);
      setAnchor(null);
      setPath(null);

      if (!restored.current) {
        restored.current = true;
        if (initial.selected) focus(initial.selected);
        if (initial.camera) set_camera_state(initial.camera);
      }
    },
  });

  const selectedId = selection.type === "Building" ? selection.data.id : null;
  useUrlSync(summary !== null, mode, filter, selectedId, timelineDay);
  const focused = filter.only_cycles ? cycle : null;

  useEffect(() => {
    if (!summary || !city.current) return;
    const a = computeAppearance(city.current, {
      mode,
      filter,
      timeline: timelineDay,
      selectedId,
      path: path?.ids ?? (focused ? closedLoop(focused.loop) : []),
    });
    set_appearance(a.colors, a.states, a.grounds, a.links);
  }, [summary, mode, filter, timelineDay, selectedId, path, focused]);

  function pick(nameOrId: string) {
    const id = city.current && resolveSymbol(city.current, nameOrId);
    if (id) focus(id);
  }

  function tracePathTo(to: string) {
    if (!city.current || !anchor) return;
    const query = {
      from_id: anchor.id,
      to_id: to,
      hops: shortestPath(city.current.adj, anchor.id, to),
    };
    setPath({ ids: query.hops, info: pathInfo(city.current, query) });
  }

  function pinAnchor(id: string) {
    setPath(null);
    if (!id) setAnchor(null);
    else if (selection.type === "Building")
      setAnchor({ id, name: selection.data.name });
  }

  function clearPath() {
    setAnchor(null);
    setPath(null);
  }

  function reset() {
    reset_camera();
    setSelection(NOTHING);
    setCycle(null);
    clearPath();
  }

  function pickCycle(id: number) {
    setSelection(NOTHING);
    clearPath();
    const loops = summary?.cycles.find((c) => c.id === id)?.loops ?? [];
    setCycle(
      id === cycle?.id || loops.length === 0
        ? null
        : { id, loops, loop: loops[0] },
    );
  }

  return (
    <div className="relative h-screen w-screen overflow-hidden bg-black">
      <canvas
        id="bevy-canvas"
        className="h-full w-full"
        onContextMenu={(e) => e.preventDefault()}
      />

      {summary && !loader.error && (
        <>
          <DistrictLabels districts={summary.districts} />
          <div className="pointer-events-none absolute bottom-2.5 left-3 z-30 font-mono text-[11px] text-white/80">
            drag: orbit; scroll: zoom; WASD: pan; click: select; Esc: clear; R:
            reset
          </div>
          <SearchBar
            summary={summary}
            busy={loader.busy}
            onPick={pick}
            onReset={reset}
            onRegenerate={(level) => {
              setSelection(NOTHING);
              void loader.regenerate(level);
            }}
          />
          <ViewControls
            summary={summary}
            mode={mode}
            filter={filter}
            links={links}
            onMode={setMode}
            onFilter={setFilter}
            onLinks={(on) => {
              setLinks(on);
              set_show_links(on);
            }}
            table={table}
            onTable={setTable}
          >
            {table && (
              <TopTable
                buildings={buildings.filter((b) =>
                  isVisible(b, filter, timelineDay),
                )}
                onPick={pick}
              />
            )}
          </ViewControls>
          <Legend summary={summary} mode={mode} />
          {filter.only_cycles && (
            <CycleList
              cycles={summary.cycles}
              active={focused?.id ?? null}
              loops={focused?.loops ?? []}
              loop={focused?.loop ?? null}
              nameOf={(id) => city.current?.buildings.get(id)?.name ?? id}
              onCycle={pickCycle}
              onLoop={(loop) => {
                setSelection(NOTHING);
                clearPath();
                setCycle(cycle && { ...cycle, loop });
              }}
            />
          )}
          <InspectorPanel
            selection={selection}
            repoUrl={repoUrl}
            commit={loader.commit}
            onClose={() => {
              setSelection(NOTHING);
              setPath(null);
            }}
            onPick={pick}
            onAnchor={pinAnchor}
            onPath={tracePathTo}
            anchorId={anchor?.id ?? null}
          />
          <PathBanner
            anchorName={anchor?.name ?? null}
            path={path?.info ?? null}
            onClear={clearPath}
          />
          <Tooltip hover={selection.type === "None" ? hover : null} />
          <Timeline
            summary={summary}
            day={timelineDay}
            onChange={setTimelineDay}
          />
          {(loader.busy || !summary.refined) && (
            <div className="absolute bottom-16 left-1/2 z-40 -translate-x-1/2 rounded border border-gray-700 bg-gray-900/95 px-3 py-1.5 text-xs text-gray-300">
              {loader.status}
            </div>
          )}
        </>
      )}

      {(!summary || loader.error) && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/80">
          <div className="w-80 rounded border border-gray-700 bg-gray-900 p-5 text-sm text-gray-200">
            <div className="font-semibold text-white">
              {owner}/{repo}
            </div>
            {loader.error ? (
              <div className="mt-2 text-red-400">{loader.error}</div>
            ) : (
              <>
                <div className="mt-2 text-gray-400">{loader.status}</div>
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
