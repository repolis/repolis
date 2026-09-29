import { useEffect, useRef, useState } from "react";

/** Mirrors the Refresh* constants in backend/internal/models/api.go. */
export const REFRESH_LEVELS = [
  {
    level: "city",
    label: "Re-analyse",
    hint: "Parse, cluster and lay out again. Keeps the checkout and cached model answers.",
  },
  {
    level: "model",
    label: "Re-ask the model",
    hint: "Also discards cached model answers, so every name and attribution is asked again.",
  },
  {
    level: "clone",
    label: "Re-clone",
    hint: "Also deletes the checkout and downloads the repository again.",
  },
] as const;

export function Regenerate({
  busy,
  onRun,
}: {
  busy: boolean;
  onRun: (level: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Close when clicking anywhere else, so the menu never sits over the canvas
  // swallowing orbit drags.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        disabled={busy}
        onClick={() => setOpen(!open)}
        title="Regenerate this city"
        className="rounded border border-gray-700 bg-gray-900/95 px-2 py-1.5 text-gray-300 hover:border-gray-500 disabled:opacity-50"
      >
        {busy ? "Working…" : "Regenerate"}
      </button>

      {open && (
        <div className="absolute right-0 z-50 mt-1 w-72 rounded border border-gray-700 bg-gray-900/95">
          {REFRESH_LEVELS.map((r) => (
            <button
              key={r.level}
              className="block w-full border-b border-gray-800 px-3 py-2 text-left last:border-b-0 hover:bg-gray-800"
              onClick={() => {
                setOpen(false);
                onRun(r.level);
              }}
            >
              <div className="font-semibold text-gray-200">{r.label}</div>
              <div className="mt-0.5 text-[10px] leading-snug text-gray-500">{r.hint}</div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
