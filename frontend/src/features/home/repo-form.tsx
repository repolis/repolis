import { useEffect, useState, type FormEvent, type RefObject } from "react";
import { AnimatePresence, motion, useAnimate } from "motion/react";

import { GLASS } from "@/shared/glass/maps";
import { useGlass } from "@/shared/glass/use-glass";
import { cn } from "@/shared/lib/cn";
import { parseRepo } from "@/shared/lib/format";
import { paneFade, spring } from "@/shared/lib/motion";
import { GlassButton } from "@/shared/ui/glass";
import { IconArrowRight, IconGithubMono } from "@/shared/ui/icons";

import type { Lookup } from "./use-repo-lookup";

/**
 * The one input on the landing page: a thick glass capsule. It reads as a
 * single address, "github.com/owner/repository"; the prefix folds away while
 * typing. The create button only slides out once GitHub confirms the
 * repository exists (or cannot be asked). Accepts "owner/repo", a github.com
 * path or a full URL; pasted links are folded down to owner/repo.
 */
export function RepoForm({
  inputRef,
  value,
  onChange,
  onSubmit,
  lookup,
}: {
  /** Owned by the page, so a suggestion can put the caret in the field. */
  inputRef: RefObject<HTMLInputElement | null>;
  value: string;
  onChange: (v: string) => void;
  onSubmit: (slug: string) => void;
  lookup: Lookup;
}) {
  const [focused, setFocused] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scope, animate] = useAnimate<HTMLDivElement>();
  const {
    attach,
    style: glassStyle,
    layers: glassLayers,
  } = useGlass<HTMLLabelElement>(GLASS.capsule, "full");

  const parsed = parseRepo(value);
  const ready = lookup.state === "found" || lookup.state === "unknown";
  const checking = lookup.state === "checking";
  const showPrefix = !focused && value.length === 0;

  // "/" focuses the field from anywhere, the way search does on GitHub.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "/" && document.activeElement !== inputRef.current) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [inputRef]);

  function handleChange(raw: string) {
    setError(null);
    const looksLikeUrl =
      /github\.com[/:]/.test(raw) || /^https?:\/\//.test(raw);
    const p = looksLikeUrl ? parseRepo(raw) : null;
    onChange(p ? `${p.owner}/${p.repo}` : raw);
  }

  function reject(message: string) {
    setError(message);
    void animate(
      scope.current,
      { x: [0, -10, 8, -5, 2, 0] },
      { duration: 0.42, ease: "easeOut" },
    );
    inputRef.current?.focus();
  }

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!parsed) {
      reject(
        value.trim()
          ? "That doesn't look like owner/repository"
          : "Paste a GitHub repository to begin",
      );
      return;
    }
    if (lookup.state === "missing") {
      reject(`No public repository at ${parsed.owner}/${parsed.repo}`);
      return;
    }
    if (lookup.state === "found") {
      onSubmit(lookup.facts.fullName);
      return;
    }
    if (lookup.state === "unknown") onSubmit(`${parsed.owner}/${parsed.repo}`);
    // Still checking: Enter waits for the answer rather than guessing.
  }

  return (
    <form onSubmit={handleSubmit} noValidate className="relative w-full">
      <div ref={scope}>
        <motion.label
          ref={attach}
          variants={paneFade}
          data-glass=""
          className={cn(
            "type-hud relative isolate flex h-[4.5rem] w-full cursor-text items-center rounded-full pr-3 pl-6 transition-colors duration-500 ease-[var(--ease-glass)]",
            focused ? "bg-white/[0.1]" : "bg-white/[0.07]",
          )}
          style={glassStyle}
        >
          {glassLayers}

          {/* Looking the repository up: a light runs along the top edge. */}
          <AnimatePresence>
            {checking && (
              <motion.span
                key="looking"
                aria-hidden
                className="pointer-events-none absolute inset-x-10 top-0 h-px overflow-hidden"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0, transition: { duration: 0.4 } }}
              >
                <motion.span
                  className="absolute inset-y-0 left-0 w-1/3 bg-gradient-to-r from-transparent via-white/90 to-transparent"
                  initial={{ x: "-100%" }}
                  animate={{ x: "300%" }}
                  transition={{
                    duration: 1.2,
                    repeat: Infinity,
                    ease: [0.45, 0, 0.55, 1],
                  }}
                />
              </motion.span>
            )}
          </AnimatePresence>

          <motion.span
            aria-hidden
            className="relative mr-3 grid size-7 shrink-0 place-items-center"
            animate={{
              color: focused || value ? "#ffffff" : "rgba(255,255,255,0.65)",
              scale: focused ? 1.06 : 1,
            }}
            transition={spring.glass}
          >
            <IconGithubMono className="size-7" />
          </motion.span>

          <AnimatePresence initial={false}>
            {showPrefix && (
              <motion.span
                key="prefix"
                aria-hidden
                initial={{ width: 0, opacity: 0 }}
                animate={{ width: "auto", opacity: 1 }}
                exit={{ width: 0, opacity: 0 }}
                transition={{ ...spring.glass, opacity: { duration: 0.2 } }}
                className="relative shrink-0 overflow-hidden whitespace-nowrap text-white/45 max-sm:hidden"
              >
                github.com/
              </motion.span>
            )}
          </AnimatePresence>

          <input
            ref={inputRef}
            value={value}
            onChange={(e) => handleChange(e.target.value)}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            spellCheck={false}
            autoCapitalize="off"
            autoComplete="off"
            aria-label="GitHub repository"
            aria-invalid={!!error || lookup.state === "missing"}
            placeholder="owner/repository"
            className="relative min-w-0 flex-1 bg-transparent text-white caret-white outline-none placeholder:text-white/50"
          />

          <AnimatePresence initial={false}>
            {ready && (
              <motion.div
                key="create"
                initial={{ width: 0, opacity: 0 }}
                animate={{ width: "auto", opacity: 1 }}
                exit={{ width: 0, opacity: 0 }}
                transition={{ ...spring.glass, opacity: { duration: 0.25 } }}
                className="relative flex shrink-0 justify-end overflow-hidden"
              >
                <motion.div
                  initial={{ x: 28 }}
                  animate={{ x: 0 }}
                  exit={{ x: 28 }}
                  transition={spring.glass}
                  className="pl-3"
                >
                  <GlassButton
                    type="submit"
                    tone="primary"
                    className="glass-nudge"
                    // Mounts long after the page's fade-in has run: its
                    // entrance is the slide above, not the inherited fade.
                    inherit={false}
                  >
                    Create
                    <span className="glass-icon grid size-5 place-items-center [&>svg]:size-full">
                      <IconArrowRight />
                    </span>
                  </GlassButton>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.label>
      </div>

      <AnimatePresence>
        {error && (
          <motion.p
            role="alert"
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={spring.glass}
            className="absolute inset-x-0 top-full mt-14 text-center text-[1.0625rem] text-[var(--color-danger)]"
          >
            {error}
          </motion.p>
        )}
      </AnimatePresence>
    </form>
  );
}
