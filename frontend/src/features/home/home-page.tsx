import { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";

import { parseRepo } from "@/shared/lib/format";
import { reveal, spring, stagger } from "@/shared/lib/motion";
import { pushRecent, readRecent } from "@/shared/lib/recent";
import { skyScenes } from "@/shared/sky/sky";
import { Logo, StatusDot } from "@/shared/ui/brand";
import { GlassButton } from "@/shared/ui/glass";
import { IconGithub } from "@/shared/ui/icons";

import { RepoForm } from "./repo-form";

/** Measured in the architecture notes: one per language. */
const EXAMPLES = ["tsoding/nothing", "gin-gonic/gin", "BurntSushi/ripgrep"];

export default function HomePage() {
  const navigate = useNavigate();
  const [leaving, setLeaving] = useState(false);
  const [draft, setDraft] = useState("");
  const [recent] = useState(readRecent);

  useEffect(() => {
    void skyScenes.landing();
  }, []);

  const target = parseRepo(draft);
  const suggestions = [
    ...recent,
    ...EXAMPLES.filter((e) => !recent.includes(e)),
  ].slice(0, 3);

  function go(slug: string) {
    const parsed = parseRepo(slug);
    if (!parsed || leaving) return;
    pushRecent(`${parsed.owner}/${parsed.repo}`);
    setLeaving(true);
    // The fall into the clouds starts here and carries on under the city
    // route, so the hand-over happens inside one continuous motion.
    void skyScenes.dive();
    window.setTimeout(() => {
      void navigate({
        to: "/city/$owner/$repo",
        params: { owner: parsed.owner, repo: parsed.repo },
      });
    }, 420);
  }

  return (
    <div className="relative z-30 h-full w-full">
      <AnimatePresence>
        {!leaving && (
          <motion.header
            key="header"
            className="pointer-events-none absolute inset-x-0 top-0 flex items-center justify-between px-10 pt-10 max-md:px-5 max-md:pt-5"
            initial="hidden"
            animate="show"
            exit={{ opacity: 0, transition: { duration: 0.3 } }}
            variants={stagger(0.06, 0.1)}
          >
            <motion.a
              href="/"
              variants={reveal}
              className="pointer-events-auto rounded-xl"
              aria-label="repolis"
            >
              <Logo />
            </motion.a>

            <motion.div
              variants={reveal}
              className="type-hud absolute top-[3.25rem] left-1/2 flex -translate-x-1/2 items-center gap-2.5 whitespace-nowrap max-md:hidden"
            >
              <StatusDot signal={target ? "live" : "idle"} className="-m-1" />
              <AnimatePresence mode="popLayout" initial={false}>
                <motion.span
                  key={target ? "ready" : "idle"}
                  className="flex items-center gap-3"
                  initial={{ opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -4 }}
                  transition={spring.glass}
                >
                  {target ? (
                    <>
                      <span className="text-white/85">Ready to build</span>
                      <span className="flex items-center gap-0.5">
                        <IconGithub className="size-6" />
                        <span className="text-white/65"> {target.owner}/</span>
                        <span className="text-white/85">{target.repo}</span>
                      </span>
                    </>
                  ) : (
                    <span className="text-white/65">Not connected</span>
                  )}
                </motion.span>
              </AnimatePresence>
            </motion.div>

            <motion.div variants={reveal} className="pointer-events-auto">
              <GlassButton
                icon={<IconGithub />}
                onClick={() =>
                  window.open("https://github.com/repolis/repolis", "_blank")
                }
              >
                <span className="max-md:hidden">Repository</span>
              </GlassButton>
            </motion.div>
          </motion.header>
        )}
      </AnimatePresence>

      <main className="absolute inset-0 flex items-center justify-center px-6">
        <AnimatePresence>
          {!leaving && (
            <motion.section
              key="hero"
              className="flex w-full max-w-[46rem] flex-col items-center"
              initial="hidden"
              animate="show"
              exit={{
                opacity: 0,
                scale: 1.04,
                transition: { duration: 0.4, ease: [0.65, 0, 0.35, 1] },
              }}
              variants={stagger(0.08, 0.25)}
            >
              <motion.div variants={reveal} className="w-full">
                <RepoForm
                  value={draft}
                  onChange={setDraft}
                  onSubmit={(slug) => go(slug)}
                />
              </motion.div>

              <motion.p
                variants={reveal}
                className="mt-6 flex flex-wrap items-center justify-center gap-x-4 gap-y-2 text-[1.0625rem]"
              >
                <span className="text-white/45">Try</span>
                {suggestions.map((slug) => (
                  <button
                    key={slug}
                    type="button"
                    onClick={() => {
                      setDraft(slug);
                      go(slug);
                    }}
                    className="text-white/65 transition-colors duration-200 hover:text-white"
                  >
                    {slug}
                  </button>
                ))}
              </motion.p>
            </motion.section>
          )}
        </AnimatePresence>
      </main>
    </div>
  );
}
