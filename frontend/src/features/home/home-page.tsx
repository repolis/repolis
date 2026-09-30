import { useEffect, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { parseRepo } from "@/shared/lib/format";
import { reveal, rise, spring, stagger } from "@/shared/lib/motion";
import { pushRecent, readRecent } from "@/shared/lib/recent";
import { returning, sky, skyScenes } from "@/shared/sky/sky";
import { GlowRing, Logo, Shimmer, StatusDot } from "@/shared/ui/brand";
import { GlassLink } from "@/shared/ui/glass";
import { IconGithub } from "@/shared/ui/icons";

import { RepoForm } from "./repo-form";
import { formatStars, useRepoLookup, type Lookup } from "./use-repo-lookup";

/** Measured in the architecture notes: one per language. */
const EXAMPLES = ["tsoding/nothing", "gin-gonic/gin", "BurntSushi/ripgrep"];

export default function HomePage() {
  const navigate = useNavigate();
  const [leaving, setLeaving] = useState(false);
  const [draft, setDraft] = useState("");
  const [recent] = useState(readRecent);
  const inputRef = useRef<HTMLInputElement>(null);
  // Arriving from a city: after the flight back (a page load, `returning`)
  // or with the browser's back button, when the sky is still the city's.
  const [fromCity] = useState(() => returning || sky.photo.get() < 0.5);

  useEffect(() => {
    if (!fromCity) {
      void skyScenes.landing();
      return;
    }
    // Open the clouds the city closed, then clear the note so a later
    // reload starts on the clear landscape.
    if (returning) skyScenes.consumeHandOff();
    else skyScenes.cover();
    void skyScenes.arrive();
  }, [fromCity]);

  // A suggestion fills the field, as if typed: it is looked up, and the
  // create button comes out once the repository is confirmed.
  function suggest(slug: string) {
    setDraft(slug);
    const input = inputRef.current;
    if (!input) return;
    input.focus({ preventScroll: true });
    requestAnimationFrame(() =>
      input.setSelectionRange(slug.length, slug.length),
    );
  }

  const target = parseRepo(draft);
  const lookup = useRepoLookup(
    target ? `${target.owner}/${target.repo}` : null,
  );
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
            className="pointer-events-none absolute inset-x-0 top-0 z-10 flex items-center justify-between px-10 pt-10 max-md:px-5 max-md:pt-5"
            initial="hidden"
            animate="show"
            exit="exit"
            variants={stagger(0.06, fromCity ? 1.1 : 0.1)}
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
              className="type-hud absolute top-[3.25rem] left-1/2 flex -translate-x-1/2 flex-col items-center gap-1.5 whitespace-nowrap max-md:hidden"
            >
              <LandingStatus lookup={lookup} />
            </motion.div>

            <motion.div variants={rise} className="pointer-events-auto">
              <GlassLink
                href="https://github.com/repolis/repolis"
                icon={<IconGithub />}
                aria-label="repolis on GitHub"
              >
                <span className="max-md:hidden">Repository</span>
              </GlassLink>
            </motion.div>
          </motion.header>
        )}
      </AnimatePresence>

      {/* Full-screen for centring only: it must not swallow clicks meant for
          the header, so only the hero itself takes the pointer. */}
      <main className="pointer-events-none absolute inset-0 flex items-center justify-center px-6">
        {/* The overlay approach from the Figma frame, local to the hero:
            the scene darkens softly behind the field and the suggestions. */}
        <motion.div
          aria-hidden
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_42%_32%_at_50%_53%,rgb(0_0_0/0.5),rgb(0_0_0/0))]"
          initial={{ opacity: 0 }}
          animate={{ opacity: leaving ? 0 : 1 }}
          transition={{ duration: leaving ? 0.4 : 1.2 }}
        />
        <AnimatePresence>
          {!leaving && (
            <motion.section
              key="hero"
              className="pointer-events-auto flex w-full max-w-[46rem] flex-col items-center"
              initial="hidden"
              animate="show"
              exit="exit"
              variants={{
                // Coming back from a city, the page waits for the clouds to
                // open before it comes in.
                ...stagger(0.08, fromCity ? 1.2 : 0.25),
                // Grows toward the viewer as the dive starts; each part
                // fades itself (see `paneFade`).
                exit: {
                  scale: 1.04,
                  transition: { duration: 0.4, ease: [0.65, 0, 0.35, 1] },
                },
              }}
            >
              <motion.div variants={rise} className="w-full">
                <RepoForm
                  inputRef={inputRef}
                  value={draft}
                  onChange={setDraft}
                  onSubmit={(slug) => go(slug)}
                  lookup={lookup}
                />
              </motion.div>

              <motion.p
                variants={reveal}
                className="mt-6 flex flex-wrap items-center justify-center gap-x-4 gap-y-2 text-[1.0625rem]"
              >
                <span className="text-white/60">Try</span>
                {suggestions.map((slug) => (
                  <button
                    key={slug}
                    type="button"
                    onClick={() => suggest(slug)}
                    className={cn(
                      "relative transition-colors duration-300 after:absolute after:inset-x-0 after:-bottom-0.5 after:h-px after:origin-left after:bg-white/60 after:transition-transform after:duration-300 after:ease-[var(--ease-glass)] hover:text-white hover:after:scale-x-100",
                      draft === slug
                        ? "text-white after:scale-x-100"
                        : "text-white/85 after:scale-x-0",
                    )}
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

/**
 * The status under the halo, as on the city page: grey until a repository
 * is typed, amber while GitHub is asked, green with its language and stars
 * once it is found, red when there is nothing there.
 */
function LandingStatus({ lookup }: { lookup: Lookup }) {
  const slug = lookup.state === "idle" ? null : lookup.slug;
  const name = lookup.state === "found" ? lookup.facts.fullName : (slug ?? "");
  const [owner, repo] = name.split("/");
  const signal =
    lookup.state === "found" || lookup.state === "unknown"
      ? "live"
      : lookup.state === "checking"
        ? "busy"
        : lookup.state === "missing"
          ? "error"
          : "idle";
  const lead = {
    idle: "Not connected",
    checking: "Looking up",
    found: "Connected to",
    unknown: "Ready to build",
    missing: "Nothing at",
  }[lookup.state];
  const note =
    lookup.state === "found"
      ? [
          lookup.facts.language,
          `${formatStars(lookup.facts.stars)} ${lookup.facts.stars === 1 ? "star" : "stars"}`,
        ]
          .filter(Boolean)
          .join(" · ")
      : lookup.state === "missing"
        ? "Check the owner and the name, or paste the link"
        : null;

  return (
    <>
      <AnimatePresence>
        {lookup.state === "found" && (
          <motion.div
            key="halo"
            className="pointer-events-none absolute top-0 left-1/2 -translate-x-1/2"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.8 }}
          >
            <GlowRing signal="live" className="!-top-[34.3125rem]" />
          </motion.div>
        )}
      </AnimatePresence>
      <div className="relative flex items-center gap-2.5">
        <StatusDot signal={signal} className="-m-1" />
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.span
            key={lead}
            layout="position"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={spring.glass}
          >
            {lookup.state === "checking" ? (
              <Shimmer>{lead}</Shimmer>
            ) : (
              <span
                className={
                  lookup.state === "idle" ? "text-white/65" : "text-white/85"
                }
              >
                {lead}
              </span>
            )}
          </motion.span>
        </AnimatePresence>
        <AnimatePresence initial={false}>
          {slug && (
            <motion.span
              key="repo"
              layout="position"
              initial={{ opacity: 0, x: -6 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -6 }}
              transition={spring.glass}
              className="ml-0.5 flex items-center gap-0.5"
            >
              <IconGithub className="size-6" />
              <span className="text-white/65"> {owner}/</span>
              <span className="text-white/85">{repo}</span>
            </motion.span>
          )}
        </AnimatePresence>
      </div>
      <AnimatePresence initial={false}>
        {note && (
          <motion.div
            key={note}
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -2 }}
            transition={spring.glass}
            className="text-[1.0625rem] text-white/65"
          >
            {note}
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
