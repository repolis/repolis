import { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { parseRepo } from "@/shared/lib/format";
import { reveal, spring, stagger } from "@/shared/lib/motion";
import { pushRecent, readRecent } from "@/shared/lib/recent";
import { skyScenes } from "@/shared/sky/sky";
import { Logo, StatusDot } from "@/shared/ui/brand";
import { Chip, IconButton, StatPill } from "@/shared/ui/glass";
import { IconCity, IconFire, IconGithub, IconPalette } from "@/shared/ui/icons";

import { LANGUAGE_COLORS } from "../render/types";
import { RepoForm } from "./repo-form";

/** Measured in the architecture notes: one per language, small to large. */
const EXAMPLES = [
  { slug: "tsoding/nothing", lang: "C" },
  { slug: "gin-gonic/gin", lang: "Go" },
  { slug: "BurntSushi/ripgrep", lang: "Rust" },
  { slug: "rgamble/libcsv", lang: "C" },
];

const HEADLINE = [
  ["Every", "repository"],
  ["is", "a", "city."],
];

export default function HomePage() {
  const navigate = useNavigate();
  const [leaving, setLeaving] = useState(false);
  const [draft, setDraft] = useState("");
  const [recent] = useState(readRecent);

  useEffect(() => {
    void skyScenes.landing();
  }, []);

  const target = parseRepo(draft);

  async function go(slug: string) {
    const parsed = parseRepo(slug);
    if (!parsed || leaving) return;
    pushRecent(`${parsed.owner}/${parsed.repo}`);
    setLeaving(true);
    // Fall into the cloud bank first; the city page picks up inside it.
    await Promise.race([
      skyScenes.dive(),
      new Promise((r) => setTimeout(r, 950)),
    ]);
    void navigate({
      to: "/city/$owner/$repo",
      params: { owner: parsed.owner, repo: parsed.repo },
    });
  }

  const quick = [
    ...recent.map((slug) => ({ slug, lang: "", recent: true })),
    ...EXAMPLES.filter((e) => !recent.includes(e.slug)).map((e) => ({
      ...e,
      recent: false,
    })),
  ].slice(0, 5);

  return (
    <div className="relative z-30 flex h-full w-full flex-col">
      <AnimatePresence>
        {!leaving && (
          <motion.header
            key="header"
            className="pointer-events-none absolute inset-x-0 top-0 flex items-start justify-between px-[3.75rem] pt-[3.75rem]"
            initial="hidden"
            animate="show"
            exit="exit"
            variants={stagger(0.08, 0.1)}
          >
            <motion.a
              href="/"
              variants={reveal}
              className="pointer-events-auto rounded-xl"
              aria-label="repolis home"
            >
              <Logo />
            </motion.a>

            <motion.div
              variants={reveal}
              className="text-lift absolute left-1/2 mt-[0.625rem] flex -translate-x-1/2 items-center gap-3 text-[1.25rem] font-semibold whitespace-nowrap"
            >
              <StatusDot signal={target ? "live" : "idle"} />
              <AnimatePresence mode="popLayout" initial={false}>
                {target ? (
                  <motion.span
                    key="ready"
                    className="flex items-center gap-3"
                    initial={{ opacity: 0, filter: "blur(6px)", y: 6 }}
                    animate={{ opacity: 1, filter: "blur(0px)", y: 0 }}
                    exit={{ opacity: 0, filter: "blur(6px)", y: -6 }}
                    transition={spring.glass}
                  >
                    <span className="text-white/85">Ready to build</span>
                    <span className="flex items-center gap-0.5">
                      <IconGithub className="size-6 text-white/85" />
                      <span className="text-white/65">{target.owner}/</span>
                      <span className="text-white/85">{target.repo}</span>
                    </span>
                  </motion.span>
                ) : (
                  <motion.span
                    key="idle"
                    className="text-white/65"
                    initial={{ opacity: 0, filter: "blur(6px)", y: 6 }}
                    animate={{ opacity: 1, filter: "blur(0px)", y: 0 }}
                    exit={{ opacity: 0, filter: "blur(6px)", y: -6 }}
                    transition={spring.glass}
                  >
                    Not connected
                  </motion.span>
                )}
              </AnimatePresence>
            </motion.div>

            <motion.div variants={reveal} className="pointer-events-auto">
              <IconButton
                label="Source on GitHub"
                size="lg"
                onClick={() =>
                  window.open("https://github.com/repolis/repolis", "_blank")
                }
              >
                <IconGithub />
              </IconButton>
            </motion.div>
          </motion.header>
        )}
      </AnimatePresence>

      {/* A soft pool of shade behind the hero keeps white type legible
          when a bright cloud drifts through. */}
      <motion.div
        aria-hidden
        className="pointer-events-none absolute inset-0"
        animate={{ opacity: leaving ? 0 : 1 }}
        transition={{ duration: 0.6 }}
        style={{
          background:
            "radial-gradient(ellipse 46% 42% at 50% 52%, rgb(16 28 44 / 0.3), rgb(16 28 44 / 0.12) 55%, transparent 80%)",
        }}
      />

      <main className="relative flex flex-1 items-center justify-center px-6">
        <AnimatePresence>
          {!leaving && (
            <motion.section
              key="hero"
              className="flex w-full max-w-[64rem] flex-col items-center text-center"
              initial="hidden"
              animate="show"
              exit={{
                opacity: 0,
                scale: 1.08,
                filter: "blur(18px)",
                transition: { duration: 0.7, ease: [0.65, 0, 0.35, 1] },
              }}
              variants={stagger(0.07, 0.25)}
            >
              <motion.div
                variants={reveal}
                className="glass mb-8 inline-flex h-9 items-center gap-2.5 rounded-full px-4 text-[0.9375rem] font-semibold text-white/80"
              >
                {(["C", "Rust", "Go"] as const).map((l, i) => (
                  <span key={l} className="flex items-center gap-1.5">
                    {i > 0 && <span className="text-white/30">·</span>}
                    <span
                      className="size-2 rounded-full"
                      style={{ background: LANGUAGE_COLORS[l] }}
                    />
                    {l}
                  </span>
                ))}
                <span className="mx-1 h-4 w-px bg-white/20" />
                <span className="text-white/60">code, rendered as a city</span>
              </motion.div>

              <h1 className="text-lift text-[clamp(3rem,5.5rem,11vw)] leading-[1.02] font-semibold tracking-[-0.04em]">
                {HEADLINE.map((line, li) => (
                  <span key={li} className="block">
                    {line.map((w, i) => (
                      <motion.span
                        key={w}
                        variants={{
                          hidden: { opacity: 0, y: 24, filter: "blur(14px)" },
                          show: {
                            opacity: 1,
                            y: 0,
                            filter: "blur(0px)",
                            transition: spring.soft,
                          },
                        }}
                        className={cn(
                          "inline-block",
                          li === 0 ? "text-white/75" : "text-white",
                        )}
                      >
                        {w}
                        {i < line.length - 1 ? "\u00a0" : ""}
                      </motion.span>
                    ))}
                  </span>
                ))}
              </h1>

              <motion.p
                variants={reveal}
                className="text-lift mt-7 max-w-[42rem] text-[1.375rem] leading-snug font-medium text-white/80"
              >
                Paste a GitHub link. Repolis parses the source, reads its
                history and raises a district for every part of the code that
                calls itself.
              </motion.p>

              <motion.div
                variants={reveal}
                className="mt-12 w-full max-w-[46rem]"
              >
                <RepoForm
                  value={draft}
                  onChange={setDraft}
                  onSubmit={(slug) => void go(slug)}
                />
              </motion.div>

              <motion.div
                variants={reveal}
                className="mt-6 flex flex-wrap items-center justify-center gap-2"
              >
                <span className="text-lift mr-1 text-[0.9375rem] font-semibold text-white/75">
                  Try
                </span>
                {quick.map((e) => (
                  <Chip
                    key={e.slug}
                    color={
                      e.recent
                        ? "rgb(255 255 255 / 0.8)"
                        : LANGUAGE_COLORS[e.lang]
                    }
                    onClick={() => {
                      setDraft(e.slug);
                      void go(e.slug);
                    }}
                    className="glass h-9 px-3.5 text-[0.9375rem] font-semibold"
                  >
                    {e.slug}
                  </Chip>
                ))}
              </motion.div>
            </motion.section>
          )}
        </AnimatePresence>
      </main>

      <AnimatePresence>
        {!leaving && (
          <motion.footer
            key="footer"
            className="pointer-events-none absolute inset-x-0 bottom-0 flex items-end justify-between px-[3.75rem] pb-[3.75rem]"
            initial="hidden"
            animate="show"
            exit="exit"
            variants={stagger(0.06, 0.6)}
          >
            <div className="flex flex-wrap gap-2.5">
              <motion.div variants={reveal}>
                <StatPill
                  tone="dark"
                  icon={<IconCity />}
                  value="Height"
                  label="· functions"
                />
              </motion.div>
              <motion.div variants={reveal}>
                <StatPill
                  tone="dark"
                  icon={<IconPalette />}
                  value="Colour"
                  label="· purpose"
                />
              </motion.div>
              <motion.div variants={reveal}>
                <StatPill
                  tone="dark"
                  icon={<IconFire />}
                  value="Glow"
                  label="· churn"
                />
              </motion.div>
            </div>
            <motion.p
              variants={reveal}
              className="text-lift hidden max-w-[22rem] text-right text-[0.9375rem] leading-snug font-medium text-white/70 lg:block"
            >
              The first city is built without a model and lands in about a
              second. The model only arrives later, to name the districts.
            </motion.p>
          </motion.footer>
        )}
      </AnimatePresence>
    </div>
  );
}
