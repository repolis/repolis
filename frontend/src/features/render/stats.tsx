import { motion } from "motion/react";

import { reveal, stagger } from "@/shared/lib/motion";
import { CountUp } from "@/shared/ui/count-up";
import { StatPill } from "@/shared/ui/glass";
import { IconCode, IconFile, IconFunction } from "@/shared/ui/icons";

import type { CitySummary } from "./types";

/** Bottom-left counters from the Figma frame: files, lines, functions. */
export function Stats({ summary }: { summary: CitySummary }) {
  const items = [
    { icon: <IconFile />, value: summary.total_files, label: "files" },
    { icon: <IconCode />, value: summary.total_loc, label: "lines of code" },
    {
      icon: <IconFunction />,
      value: summary.total_methods,
      label: "functions",
    },
  ];
  return (
    <motion.div
      className="hud-dim absolute bottom-[3.75rem] left-[3.75rem] z-40 flex gap-2.5"
      initial="hidden"
      animate="show"
      variants={stagger(0.08, 0.2)}
    >
      {items.map((it, i) => (
        <motion.div key={it.label} variants={reveal}>
          <StatPill
            icon={it.icon}
            value={<CountUp value={it.value} delay={0.3 + i * 0.08} />}
            label={it.label}
          />
        </motion.div>
      ))}
    </motion.div>
  );
}
