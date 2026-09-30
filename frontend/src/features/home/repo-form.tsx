import { useEffect, useRef, useState, type FormEvent } from "react";
import { AnimatePresence, motion, useAnimate } from "motion/react";

import { cn } from "@/shared/lib/cn";
import { parseRepo } from "@/shared/lib/format";
import { spring } from "@/shared/lib/motion";
import { GlassButton, Kbd } from "@/shared/ui/glass";
import { IconArrowRight, IconDanger, IconGithub } from "@/shared/ui/icons";

/**
 * The one input on the landing page. Accepts "owner/repo", a github.com path
 * or a full URL (pasted links are folded down to owner/repo as they land).
 */
export function RepoForm({
  value,
  onChange,
  onSubmit,
}: {
  value: string;
  onChange: (v: string) => void;
  onSubmit: (slug: string) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [focused, setFocused] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scope, animate] = useAnimate<HTMLDivElement>();

  const parsed = parseRepo(value);

  // "/" focuses the field from anywhere, the way search does on GitHub.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "/" && document.activeElement !== inputRef.current) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    inputRef.current?.focus({ preventScroll: true });
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function handleChange(raw: string) {
    setError(null);
    // Fold a pasted URL down to owner/repo, but leave partial typing alone.
    const looksLikeUrl =
      /github\.com[/:]/.test(raw) || /^https?:\/\//.test(raw);
    const p = looksLikeUrl ? parseRepo(raw) : null;
    onChange(p ? `${p.owner}/${p.repo}` : raw);
  }

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!parsed) {
      setError(
        value.trim()
          ? "That doesn't look like owner/repository"
          : "Paste a GitHub repository to begin",
      );
      void animate(
        scope.current,
        { x: [0, -10, 8, -5, 2, 0] },
        { duration: 0.42, ease: "easeOut" },
      );
      inputRef.current?.focus();
      return;
    }
    onSubmit(`${parsed.owner}/${parsed.repo}`);
  }

  return (
    <form onSubmit={handleSubmit} noValidate className="relative w-full">
      <div ref={scope}>
        <motion.label
          animate={{
            scale: focused ? 1.015 : 1,
            boxShadow: focused
              ? "0 0 0 0.375rem rgb(255 255 255 / 0.08), 0 1.5rem 4rem -1.5rem rgb(0 0 0 / 0.5), 0 0 3rem rgb(255 255 255 / 0.12)"
              : "0 0 0 0rem rgb(255 255 255 / 0), 0 1rem 3rem -1.5rem rgb(0 0 0 / 0.45), 0 0 0rem rgb(255 255 255 / 0)",
          }}
          transition={spring.glass}
          className="glass glass-strong flex h-[4.5rem] w-full cursor-text items-center gap-3 rounded-full pr-2 pl-6 text-left"
        >
          <IconGithub
            className={cn(
              "size-7 shrink-0 transition-colors duration-300",
              parsed ? "text-white" : "text-white/55",
            )}
          />
          <span className="hidden shrink-0 text-[1.375rem] font-semibold text-white/40 sm:inline">
            github.com/
          </span>
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
            aria-invalid={!!error}
            placeholder="owner/repository"
            className="min-w-0 flex-1 bg-transparent text-[1.375rem] font-semibold text-white caret-white outline-none placeholder:text-white/35"
          />
          <AnimatePresence initial={false}>
            {!value && !focused && (
              <motion.span
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="hidden sm:block"
              >
                <Kbd>/</Kbd>
              </motion.span>
            )}
          </AnimatePresence>
          <GlassButton
            type="submit"
            tone={parsed ? "primary" : "clear"}
            size="lg"
            className="h-14 px-6 text-[1.125rem]"
          >
            Build city
            <motion.span
              animate={{ x: parsed ? 3 : 0 }}
              transition={spring.snappy}
              className="grid size-5 place-items-center [&>svg]:size-full"
            >
              <IconArrowRight />
            </motion.span>
          </GlassButton>
        </motion.label>
      </div>

      <AnimatePresence>
        {error && (
          <motion.p
            role="alert"
            initial={{ opacity: 0, y: -6, filter: "blur(6px)" }}
            animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            exit={{ opacity: 0, y: -4, filter: "blur(6px)" }}
            transition={spring.glass}
            className="text-lift absolute inset-x-0 top-full mt-3 flex items-center justify-center gap-2 text-[0.9375rem] font-semibold text-[var(--color-danger)]"
          >
            <IconDanger className="size-4" />
            {error}
          </motion.p>
        )}
      </AnimatePresence>
    </form>
  );
}
