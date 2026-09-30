import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { spring } from "@/shared/lib/motion";
import { GlassPanel, Kbd } from "@/shared/ui/glass";
import { IconBox, IconFile, IconSearch } from "@/shared/ui/icons";

import type { CitySummary, IndexEntry } from "./types";

const isMac =
  typeof navigator !== "undefined" &&
  /Mac|iPhone|iPad/.test(navigator.platform);

function rank(summary: CitySummary | null, query: string): IndexEntry[] {
  if (!summary || query.trim().length < 2) return [];
  const q = query.trim().toLowerCase();
  return summary.index
    .map((e) => {
      const n = e.name.toLowerCase();
      if (n === q) return { e, score: 0 };
      if (n.startsWith(q)) return { e, score: 1 };
      if (n.includes(q)) return { e, score: 2 };
      return null;
    })
    .filter((r): r is { e: IndexEntry; score: number } => r !== null)
    .sort((a, b) => a.score - b.score || a.e.name.length - b.e.name.length)
    .slice(0, 12)
    .map((r) => r.e);
}

/** Highlights the matched part of a name. */
function Match({ name, query }: { name: string; query: string }) {
  const q = query.trim().toLowerCase();
  const i = q ? name.toLowerCase().indexOf(q) : -1;
  if (i < 0) return <>{name}</>;
  return (
    <>
      {name.slice(0, i)}
      <span className="text-white">{name.slice(i, i + q.length)}</span>
      {name.slice(i + q.length)}
    </>
  );
}

/** Makes the city addressable: without it a named symbol can only be found by
 * flying around by hand. Opens with ⌘K, Ctrl+K or "/". */
export function SearchPalette({
  summary,
  onPick,
}: {
  summary: CitySummary | null;
  onPick: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const results = useMemo(() => rank(summary, query), [summary, query]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing =
        document.activeElement instanceof HTMLInputElement ||
        document.activeElement instanceof HTMLTextAreaElement;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      } else if (e.key === "/" && !typing) {
        e.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function close() {
    setOpen(false);
    setQuery("");
    setActive(0);
  }

  function pick(entry: IndexEntry | undefined) {
    if (!entry) return;
    onPick(entry.id);
    close();
  }

  return (
    <>
      <motion.button
        type="button"
        onClick={() => setOpen(true)}
        whileHover={{ scale: 1.02 }}
        whileTap={{ scale: 0.97 }}
        transition={spring.snappy}
        className="glass flex h-12 w-[17rem] items-center gap-2.5 rounded-full pr-2 pl-4 text-left text-[1rem] font-semibold text-white/60 hover:text-white/85 max-[100rem]:w-auto max-md:w-12 max-md:justify-center max-md:p-0"
        aria-label="Search symbols"
      >
        <IconSearch className="size-5 shrink-0 text-white/80" />
        <span className="flex-1 truncate max-[100rem]:hidden">
          Search symbols
        </span>
        <span className="flex gap-1 max-md:hidden">
          <Kbd>{isMac ? "⌘" : "Ctrl"}</Kbd>
          <Kbd>K</Kbd>
        </span>
      </motion.button>

      <AnimatePresence>
        {open && (
          <motion.div
            key="palette"
            className="fixed inset-0 z-50 flex items-start justify-center px-4 pt-[16vh]"
            initial={{ backgroundColor: "rgb(6 10 18 / 0)" }}
            animate={{ backgroundColor: "rgb(6 10 18 / 0.28)" }}
            exit={{ backgroundColor: "rgb(6 10 18 / 0)" }}
            transition={{ duration: 0.3 }}
            onPointerDown={(e) => {
              if (e.target === e.currentTarget) close();
            }}
          >
            <GlassPanel
              strong
              className="w-full max-w-[40rem]"
              initial={{
                opacity: 0,
                y: -16,
                scale: 0.96,
                filter: "blur(12px)",
              }}
              animate={{
                opacity: 1,
                y: 0,
                scale: 1,
                filter: "blur(0px)",
                transitionEnd: { filter: "none" },
              }}
              exit={{ opacity: 0, y: -10, scale: 0.97, filter: "blur(10px)" }}
              transition={spring.glass}
            >
              <div className="flex items-center gap-3 border-b border-white/10 px-6 py-5">
                <IconSearch className="size-6 shrink-0 text-white/70" />
                <input
                  ref={inputRef}
                  autoFocus
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setActive(0);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "ArrowDown") {
                      e.preventDefault();
                      setActive((a) => Math.min(results.length - 1, a + 1));
                    } else if (e.key === "ArrowUp") {
                      e.preventDefault();
                      setActive((a) => Math.max(0, a - 1));
                    } else if (e.key === "Enter") {
                      pick(results[active]);
                    } else if (e.key === "Escape") {
                      close();
                    }
                  }}
                  placeholder="Find a type, module or file…"
                  spellCheck={false}
                  className="min-w-0 flex-1 bg-transparent text-[1.25rem] font-semibold text-white outline-none placeholder:text-white/35"
                />
                <Kbd>esc</Kbd>
              </div>

              <div className="scrollbar-glass max-h-[26rem] overflow-y-auto p-2">
                {results.length === 0 ? (
                  <div className="px-4 py-8 text-center text-[0.9375rem] font-semibold text-white/45">
                    {query.trim().length < 2
                      ? `${summary?.index.length.toLocaleString("en-US") ?? 0} buildings indexed. Type two letters to search.`
                      : "Nothing in this city by that name."}
                  </div>
                ) : (
                  results.map((r, i) => (
                    <button
                      key={r.id}
                      type="button"
                      onPointerEnter={() => setActive(i)}
                      onClick={() => pick(r)}
                      className="relative flex w-full items-center gap-3 rounded-2xl px-4 py-3 text-left"
                    >
                      {i === active && (
                        <motion.span
                          layoutId="search-active"
                          transition={spring.snappy}
                          className="absolute inset-0 rounded-2xl bg-white/14 shadow-[inset_0_1px_0_rgb(255_255_255/0.18)]"
                        />
                      )}
                      <span className="relative grid size-5 shrink-0 place-items-center text-white/60 [&>svg]:size-full">
                        {r.kind === "module" ? <IconFile /> : <IconBox />}
                      </span>
                      <span
                        className={cn(
                          "relative flex-1 truncate font-mono text-[1rem]",
                          i === active ? "text-white/90" : "text-white/65",
                        )}
                      >
                        <Match name={r.name} query={query} />
                      </span>
                      <span className="relative shrink-0 truncate text-[0.8125rem] font-semibold text-white/45">
                        {r.district}
                      </span>
                    </button>
                  ))
                )}
              </div>

              <div className="flex items-center gap-4 border-t border-white/10 px-6 py-3 text-[0.75rem] font-semibold text-white/45">
                <span className="flex items-center gap-1.5">
                  <Kbd>↑</Kbd>
                  <Kbd>↓</Kbd> to move
                </span>
                <span className="flex items-center gap-1.5">
                  <Kbd>↵</Kbd> to fly there
                </span>
              </div>
            </GlassPanel>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
