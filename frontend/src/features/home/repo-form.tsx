import { useEffect, useRef, useState, type FormEvent } from "react";
import { AnimatePresence, motion, useAnimate } from "motion/react";

import { GLASS } from "@/shared/glass/maps";
import { useGlass } from "@/shared/glass/use-glass";
import { cn } from "@/shared/lib/cn";
import { parseRepo } from "@/shared/lib/format";
import { spring } from "@/shared/lib/motion";
import { GlassButton } from "@/shared/ui/glass";
import { IconArrowRight, IconGithub } from "@/shared/ui/icons";

/**
 * The one input on the landing page: a thick glass capsule with the create
 * button inside it. Accepts "owner/repo", a github.com path or a full URL;
 * pasted links are folded down to owner/repo as they land.
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
  const {
    attach,
    style: glassStyle,
    layers: glassLayers,
  } = useGlass<HTMLLabelElement>(GLASS.capsule, "full");

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
        <label
          ref={attach}
          className={cn(
            "type-hud relative isolate flex h-[4.5rem] w-full cursor-text items-center gap-3 rounded-full pr-3 pl-6 transition-colors duration-300",
            focused ? "bg-white/[0.1]" : "bg-white/[0.07]",
          )}
          style={glassStyle}
        >
          {glassLayers}
          <IconGithub className="relative size-7 shrink-0" />
          <span className="relative shrink-0 text-white/55 max-sm:hidden">
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
            className="relative min-w-0 flex-1 bg-transparent text-white caret-white outline-none placeholder:text-white/50"
          />
          <GlassButton type="submit" tone="primary" className="relative">
            Create
            <motion.span
              animate={{ x: parsed ? 2 : 0 }}
              transition={spring.snappy}
              className="grid size-5 place-items-center [&>svg]:size-full"
            >
              <IconArrowRight />
            </motion.span>
          </GlassButton>
        </label>
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
