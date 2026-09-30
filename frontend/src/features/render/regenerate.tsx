import { useState } from "react";
import { motion } from "motion/react";

import { IconButton } from "@/shared/ui/glass";
import { IconCpu, IconRefresh, IconRestart } from "@/shared/ui/icons";
import { MenuItem, Popover } from "@/shared/ui/popover";

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

const ICONS = {
  city: <IconRefresh />,
  model: <IconCpu />,
  clone: <IconRestart />,
} as const;

export function Regenerate({
  busy,
  onRun,
}: {
  busy: boolean;
  onRun: (level: string) => void;
}) {
  const [open, setOpen] = useState(false);

  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      className="w-[20rem]"
      trigger={
        <IconButton
          size="lg"
          label={busy ? "Working…" : "Regenerate this city"}
          disabled={busy}
          active={open}
          onClick={() => setOpen(!open)}
        >
          <motion.span
            className="grid place-items-center"
            animate={{ rotate: busy ? 360 : 0 }}
            transition={
              busy
                ? { duration: 1.1, repeat: Infinity, ease: "linear" }
                : { duration: 0.3 }
            }
          >
            <IconRefresh className="size-5" />
          </motion.span>
        </IconButton>
      }
    >
      <div role="menu" className="flex flex-col">
        <div className="px-3.5 pt-2 pb-1.5 text-[0.75rem] font-semibold tracking-wide text-white/45 uppercase">
          Regenerate
        </div>
        {REFRESH_LEVELS.map((r) => (
          <MenuItem
            key={r.level}
            layoutGroup="regen"
            icon={ICONS[r.level]}
            title={r.label}
            hint={r.hint}
            onSelect={() => {
              setOpen(false);
              onRun(r.level);
            }}
          />
        ))}
      </div>
    </Popover>
  );
}
