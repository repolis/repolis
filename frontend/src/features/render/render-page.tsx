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

import { computeAppearance, shortestPath } from "./appearance";
import {
  buildCity,
  hoverInfo,
  pathInfo,
  resolveSymbol,
  selection as toSelection,
  type City,
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
import { decodeView } from "./url-state";
import { useCityLoader } from "./use-city-loader";
import { useEngineEvents } from "./use-engine-events";
import { useUrlSync } from "./use-url-sync";
import { ViewControls } from "./view-controls";

const NOTHING: SelectPayload = { type: "None" };

export default function RenderPage() {
  const { owner, repo } = useParams({ from: "/city/$owner/$repo" });
  const repoUrl = `https://github.com/${owner}/${repo}`;

  // Selection and camera from the link wait for the city.
  const [initial] = useState(() => decodeView(window.location.hash));
  const [mode, setMode] = useState(initial.mode);
  const [filter, setFilter] = useState(initial.filter);
  const [timelineDay, setTimelineDay] = useState(initial.timeline);
  const [links, setLinks] = useState(false);

  const [summary, setSummary] = useState<CitySummary | null>(null);
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

  const loader = useCityLoader(repoUrl, (raw: RawCity) => {
    city.current = buildCity(raw);
    load_city_data(raw);
  });

  useEngineEvents({
    hover: (t) => setHover(city.current && hoverInfo(city.current, t)),
    select: (t) => {
      setSelection(city.current ? toSelection(city.current, t) : NOTHING);
      setPath(null);
    },
    city: () => {
      if (!city.current) return;
      setSummary(city.current.summary);
      // Ids may not survive a reload; colour, filter and timeline do.
      setAnchor(null);
      setPath(null);
      // Once only: the refinement pass sends a second city, which must not yank the camera back.
      if (!restored.current) {
        restored.current = true;
        if (initial.selected) focus(initial.selected);
        // Queued after the focus, which moves the camera itself.
        if (initial.camera) set_camera_state(initial.camera);
      }
    },
  });

  const selectedId = selection.type === "Building" ? selection.data.id : null;
  useUrlSync(summary !== null, mode, filter, selectedId, timelineDay);

  useEffect(() => {
    if (!summary || !city.current) return;
    const a = computeAppearance(city.current, {
      mode,
      filter,
      timeline: timelineDay,
      selectedId,
      path: path?.ids ?? [],
    });
    set_appearance(a.colors, a.states, a.grounds, a.links);
  }, [summary, mode, filter, timelineDay, selectedId, path]);

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
    clearPath();
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
            drag: orbit · scroll: zoom · WASD: pan · click: select · Esc: clear
            · R: reset
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
          />
          <Legend summary={summary} mode={mode} />
          <InspectorPanel
            selection={selection}
            repoUrl={repoUrl}
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
