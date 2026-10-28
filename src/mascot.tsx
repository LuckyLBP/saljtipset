/**
 * Bubbe, Säljtipset's mascot: the speech bubble from the logo with a face and a headset.
 *
 * Drawn as SVG and animated with CSS (see "Bubbe" in index.css), one loop per mood. With
 * "Reduce motion" turned on in the OS, Bubbe holds still.
 */

export type Mood = "idle" | "listening" | "thinking" | "cheer";

export function Bubbe({ mood = "idle", size = 64, className = "" }: { mood?: Mood; size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 120 120"
      aria-hidden
      className={`bubbe bubbe-${mood} ${size < 40 ? "bubbe-small" : ""} overflow-visible ${className}`}
    >
      <defs>
        <linearGradient id="bubbe-body" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="1" stopColor="#dff5ee" />
        </linearGradient>
        <radialGradient id="bubbe-glow" cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor="#34c4a0" stopOpacity="0.3" />
          <stop offset="1" stopColor="#34c4a0" stopOpacity="0" />
        </radialGradient>
      </defs>

      {/* soft glow and ground shadow */}
      <ellipse className="bubbe-shadow" cx="60" cy="110" rx="30" ry="5" fill="#0b7a68" opacity="0.18" />
      <circle cx="60" cy="62" r="56" fill="url(#bubbe-glow)" />

      <g className="bubbe-figure">
        {/* the bubble */}
        <path
          d="M34 26h52a20 20 0 0 1 20 20v22a20 20 0 0 1-20 20H56l-18 15 3-15h-7a20 20 0 0 1-20-20V46a20 20 0 0 1 20-20Z"
          fill="url(#bubbe-body)"
          stroke="#0b7a68"
          strokeOpacity="0.18"
          strokeWidth="1.5"
        />
        {/* glass highlight */}
        <path d="M30 36c4-5 9-6 16-6" stroke="#fff" strokeWidth="4" strokeLinecap="round" opacity="0.9" fill="none" />

        {/* headset */}
        <g className="bubbe-headset">
          <path d="M26 50c0-22 15-34 34-34s34 12 34 34" fill="none" stroke="#0b5f52" strokeWidth="5" strokeLinecap="round" />
          <rect x="18" y="44" width="11" height="20" rx="5.5" fill="#0b7a68" />
          <rect x="91" y="44" width="11" height="20" rx="5.5" fill="#0b7a68" />
          <path d="M24 63c2 10 10 15 22 15" fill="none" stroke="#0b5f52" strokeWidth="3" strokeLinecap="round" />
          <circle cx="47" cy="78" r="3.5" fill="#0b5f52" />
        </g>

        {/* face */}
        <g className="bubbe-face">
          <g className="bubbe-eyes">
            <ellipse cx="48" cy="54" rx="5.5" ry="7.5" fill="#123d36" />
            <ellipse cx="72" cy="54" rx="5.5" ry="7.5" fill="#123d36" />
            <circle cx="50" cy="51" r="2" fill="#fff" />
            <circle cx="74" cy="51" r="2" fill="#fff" />
          </g>
          <ellipse cx="40" cy="66" rx="5" ry="3" fill="#f59a8c" opacity="0.45" />
          <ellipse cx="80" cy="66" rx="5" ry="3" fill="#f59a8c" opacity="0.45" />
          <path className="bubbe-mouth" d="M53 66c4 5 10 5 14 0" fill="none" stroke="#123d36" strokeWidth="3" strokeLinecap="round" />
        </g>
      </g>

      {/* the spark: the "tip" */}
      <g className="bubbe-spark">
        <path d="M100 8c1.2 7 4 9.8 11 11-7 1.2-9.8 4-11 11-1.2-7-4-9.8-11-11 7-1.2 9.8-4 11-11Z" fill="#34c4a0" />
      </g>

      {/* celebration sparkles */}
      <g className="bubbe-confetti" fill="#34c4a0">
        <path d="M14 20c.6 3.5 2 4.9 5.5 5.5-3.5.6-4.9 2-5.5 5.5-.6-3.5-2-4.9-5.5-5.5 3.5-.6 4.9-2 5.5-5.5Z" />
        <circle cx="108" cy="44" r="2.5" />
        <circle cx="10" cy="62" r="2" />
        <path d="M24 4c.4 2.3 1.3 3.2 3.6 3.6-2.3.4-3.2 1.3-3.6 3.6-.4-2.3-1.3-3.2-3.6-3.6 2.3-.4 3.2-1.3 3.6-3.6Z" />
      </g>
    </svg>
  );
}
