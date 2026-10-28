import { useEffect, useRef, useState } from "react";

/**
 * Bubbe, the mascot: a round bumblebee with a headset, in honey and charcoal.
 *
 * Drawn as SVG and animated with CSS (see "Bubbe" in index.css), one loop per mood. Every
 * mood is the same drawing with different parts shown and moved, so switching is instant.
 * With `interactive`, the eyes follow the cursor and a click makes Bubbe giggle. With
 * "Reduce motion" turned on in the OS, Bubbe holds still.
 */

export type Mood =
  | "idle" // breathes, blinks
  | "listening" // nods along while transcribing
  | "thinking" // antennae wiggle, a spark circles: the AI is working
  | "cheer" // bounces with sparkles: tips are ready
  | "hello" // happy squint and a wing wave: greetings
  | "surprised" // big eyes, O-mouth: a file is dragged over the window
  | "oops" // frown and a sweat drop: something went wrong
  | "sleepy" // closed eyes and Zzz: nothing has happened for a while
  | "flying" // buzzing wings and speed lines: downloading
  | "love"; // heart eyes: a job well done

const reducedMotion = () =>
  typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

export function Bubbe({
  mood = "idle",
  size = 64,
  interactive = false,
  className = "",
}: {
  mood?: Mood;
  size?: number;
  /** Eyes follow the cursor and a click makes Bubbe giggle. For the larger, lone Bubbe. */
  interactive?: boolean;
  className?: string;
}) {
  const ref = useRef<SVGSVGElement>(null);
  const [giggle, setGiggle] = useState(0);
  const shown: Mood = giggle ? "love" : mood;
  // Clip-path ids must be unique per instance on the page.
  const clip = `bubbe-stripes-${size}-${mood}-${interactive ? "i" : "s"}`;

  // Eyes follow the cursor: a small shift of the pupils toward it, clamped to 2.5 units.
  useEffect(() => {
    if (!interactive || reducedMotion()) return;
    let frame = 0;
    const move = (e: MouseEvent) => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const el = ref.current;
        if (!el) return;
        const r = el.getBoundingClientRect();
        const dx = e.clientX - (r.left + r.width / 2);
        const dy = e.clientY - (r.top + r.height * 0.52);
        const dist = Math.hypot(dx, dy) || 1;
        const reach = Math.min(1, dist / 400) * 2.5;
        el.style.setProperty("--look-x", `${((dx / dist) * reach).toFixed(2)}px`);
        el.style.setProperty("--look-y", `${((dy / dist) * reach).toFixed(2)}px`);
      });
    };
    window.addEventListener("mousemove", move);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("mousemove", move);
    };
  }, [interactive]);

  useEffect(() => {
    if (!giggle) return;
    const t = setTimeout(() => setGiggle(0), 1500);
    return () => clearTimeout(t);
  }, [giggle]);

  return (
    <svg
      ref={ref}
      width={size}
      height={size}
      viewBox="0 0 120 120"
      aria-hidden
      onClick={interactive ? () => setGiggle((g) => g + 1) : undefined}
      className={`bubbe bubbe-${shown} ${size < 40 ? "bubbe-small" : ""} ${interactive ? "bubbe-interactive" : ""} overflow-visible ${className}`}
    >
      <defs>
        <clipPath id={clip}>
          <ellipse cx="60" cy="70" rx="38" ry="36" />
        </clipPath>
      </defs>

      <ellipse className="bubbe-shadow" cx="60" cy="113" rx="28" ry="4.5" fill="#1f1a17" opacity="0.16" />

      {/* speed lines while flying */}
      <g className="bubbe-motion" stroke="#1f1a17" strokeOpacity="0.3" strokeWidth="2.4" strokeLinecap="round">
        <path d="M8 58h10M4 70h12M8 82h9" />
      </g>

      <g className="bubbe-figure" key={giggle}>
        {/* wings, behind the body */}
        <ellipse className="bubbe-wing bubbe-wing-l" cx="42" cy="28" rx="14" ry="20" fill="#ffffff" fillOpacity="0.85" stroke="#1f1a17" strokeOpacity="0.22" strokeWidth="1.5" />
        <ellipse className="bubbe-wing bubbe-wing-r" cx="78" cy="28" rx="14" ry="20" fill="#ffffff" fillOpacity="0.85" stroke="#1f1a17" strokeOpacity="0.22" strokeWidth="1.5" />

        {/* antennae */}
        <g className="bubbe-antennae">
          <path d="M50 36c-2-8-6-13-12-16M70 36c2-8 6-13 12-16" fill="none" stroke="#1f1a17" strokeWidth="2.6" strokeLinecap="round" />
          <circle cx="37" cy="19" r="3.6" fill="#1f1a17" />
          <circle cx="83" cy="19" r="3.6" fill="#1f1a17" />
        </g>

        {/* body with stripes */}
        <ellipse cx="60" cy="70" rx="38" ry="36" fill="#ffc53d" />
        <g clipPath={`url(#${clip})`}>
          <rect x="10" y="86" width="100" height="8" fill="#1f1a17" />
          <rect x="10" y="100" width="100" height="8" fill="#1f1a17" />
        </g>

        {/* headset */}
        <g className="bubbe-headset">
          <path d="M25 66c0-21 15-33 35-33s35 12 35 33" fill="none" stroke="#1f1a17" strokeWidth="3.5" strokeLinecap="round" />
          <rect x="18" y="60" width="10" height="17" rx="5" fill="#e5486b" />
          <rect x="92" y="60" width="10" height="17" rx="5" fill="#e5486b" />
          <path className="bubbe-mic" d="M23 76c2 8 9 12 19 12" fill="none" stroke="#1f1a17" strokeWidth="2.4" strokeLinecap="round" />
          <circle className="bubbe-mic" cx="43" cy="88" r="3" fill="#e5486b" />
        </g>

        {/* face: one set of eyes and mouths per expression, shown by mood */}
        <g className="bubbe-face">
          <g className="bubbe-look">
            <g className="bubbe-eyes">
              <ellipse cx="48" cy="62" rx="5" ry="7" fill="#1f1a17" />
              <ellipse cx="72" cy="62" rx="5" ry="7" fill="#1f1a17" />
              <circle cx="50" cy="59" r="1.9" fill="#ffffff" />
              <circle cx="74" cy="59" r="1.9" fill="#ffffff" />
            </g>
          </g>
          <g className="bubbe-eyes-happy" fill="none" stroke="#1f1a17" strokeWidth="2.8" strokeLinecap="round">
            <path d="M43 64c2-5 8-5 10 0M67 64c2-5 8-5 10 0" />
          </g>
          <g className="bubbe-eyes-sleepy" fill="none" stroke="#1f1a17" strokeWidth="2.6" strokeLinecap="round">
            <path d="M43 61c2 4 8 4 10 0M67 61c2 4 8 4 10 0" />
          </g>
          <g className="bubbe-eyes-heart" fill="#e5486b">
            <path d="M48 68c-5.5-3.6-7.5-6.2-7.5-8.8a3.6 3.6 0 0 1 7.5-1.4 3.6 3.6 0 0 1 7.5 1.4c0 2.6-2 5.2-7.5 8.8Z" />
            <path d="M72 68c-5.5-3.6-7.5-6.2-7.5-8.8a3.6 3.6 0 0 1 7.5-1.4 3.6 3.6 0 0 1 7.5 1.4c0 2.6-2 5.2-7.5 8.8Z" />
          </g>
          <ellipse cx="39" cy="73" rx="5" ry="3" fill="#e5486b" opacity="0.45" />
          <ellipse cx="81" cy="73" rx="5" ry="3" fill="#e5486b" opacity="0.45" />
          <path className="bubbe-mouth" d="M54 73c3 4 9 4 12 0" fill="none" stroke="#1f1a17" strokeWidth="2.6" strokeLinecap="round" />
          <ellipse className="bubbe-mouth-o" cx="60" cy="76" rx="3.4" ry="4.4" fill="#1f1a17" />
          <path className="bubbe-mouth-frown" d="M54 78c3-4 9-4 12 0" fill="none" stroke="#1f1a17" strokeWidth="2.6" strokeLinecap="round" />
        </g>

        {/* a sweat drop when something goes wrong */}
        <path className="bubbe-sweat" d="M91 42c3.2 4.4 4.4 6.6 4.4 8.6a4.4 4.4 0 0 1-8.8 0c0-2 1.2-4.2 4.4-8.6Z" fill="#8fd3ff" stroke="#1f1a17" strokeOpacity="0.2" />
      </g>

      {/* the spark: an idea while thinking */}
      <g className="bubbe-spark">
        <path d="M104 10c1 6 3.4 8.4 9.4 9.4-6 1-8.4 3.4-9.4 9.4-1-6-3.4-8.4-9.4-9.4 6-1 8.4-3.4 9.4-9.4Z" fill="#e5486b" />
      </g>

      {/* Zzz while sleepy */}
      <g className="bubbe-zzz" fill="currentColor" fontFamily="'Baloo 2', ui-rounded, sans-serif" fontWeight="800">
        <text className="bubbe-z1" x="92" y="34" fontSize="11">z</text>
        <text className="bubbe-z2" x="100" y="22" fontSize="14">z</text>
        <text className="bubbe-z3" x="109" y="8" fontSize="17">Z</text>
      </g>

      {/* floating hearts */}
      <g className="bubbe-hearts" fill="#e5486b">
        <path className="bubbe-h1" d="M18 30c-4-2.6-5.5-4.5-5.5-6.4a2.6 2.6 0 0 1 5.5-1 2.6 2.6 0 0 1 5.5 1c0 1.9-1.5 3.8-5.5 6.4Z" />
        <path className="bubbe-h2" d="M102 34c-3.2-2.1-4.4-3.6-4.4-5.1a2.1 2.1 0 0 1 4.4-.8 2.1 2.1 0 0 1 4.4.8c0 1.5-1.2 3-4.4 5.1Z" />
      </g>

      {/* celebration */}
      <g className="bubbe-confetti">
        <path d="M14 26c.6 3.5 2 4.9 5.5 5.5-3.5.6-4.9 2-5.5 5.5-.6-3.5-2-4.9-5.5-5.5 3.5-.6 4.9-2 5.5-5.5Z" fill="#ffc53d" />
        <circle cx="108" cy="48" r="2.6" fill="#e5486b" />
        <circle cx="10" cy="70" r="2.2" fill="#e5486b" />
        <path d="M100 82c.4 2.3 1.3 3.2 3.6 3.6-2.3.4-3.2 1.3-3.6 3.6-.4-2.3-1.3-3.2-3.6-3.6 2.3-.4 3.2-1.3 3.6-3.6Z" fill="#ffc53d" />
      </g>
    </svg>
  );
}
