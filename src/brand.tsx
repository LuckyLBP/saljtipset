/**
 * Säljtipset brand: a speech bubble holding a spark (the tip), in "tipset-grön".
 * The same mark is the app icon (src-tauri/icons), drawn here as SVG for the UI.
 */

export function Mark({ size = 28 }: { size?: number }) {
  const id = `mk-${size}`;
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden>
      <defs>
        <linearGradient id={`${id}-bg`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#3fd1ae" />
          <stop offset="1" stopColor="#0a6656" />
        </linearGradient>
        <linearGradient id={`${id}-sheen`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.35" />
          <stop offset="0.55" stopColor="#fff" stopOpacity="0" />
        </linearGradient>
      </defs>
      <rect x="2" y="2" width="60" height="60" rx="15" fill={`url(#${id}-bg)`} />
      <rect x="2" y="2" width="60" height="60" rx="15" fill={`url(#${id}-sheen)`} />
      <path
        d="M20 15h24a10 10 0 0 1 10 10v10a10 10 0 0 1-10 10H29l-9.5 7.5L21 45h-1a10 10 0 0 1-10-10V25a10 10 0 0 1 10-10Z"
        fill="#fff"
      />
      <path d="M32 18.5c.9 6.6 3.9 9.6 10.5 10.5-6.6.9-9.6 3.9-10.5 10.5-.9-6.6-3.9-9.6-10.5-10.5 6.6-.9 9.6-3.9 10.5-10.5Z" fill="#0b7a68" />
      <path d="M43 17.5c.3 2.2 1.3 3.2 3.5 3.5-2.2.3-3.2 1.3-3.5 3.5-.3-2.2-1.3-3.2-3.5-3.5 2.2-.3 3.2-1.3 3.5-3.5Z" fill="#34c4a0" />
    </svg>
  );
}

export function Wordmark({ size = 17 }: { size?: number }) {
  return (
    <span className="font-display font-semibold tracking-[-0.02em] text-ink" style={{ fontSize: size }}>
      Säljtipset
    </span>
  );
}

export function Logo({ mark = 26, text = 16 }: { mark?: number; text?: number }) {
  return (
    <span className="inline-flex items-center gap-2">
      <Mark size={mark} />
      <Wordmark size={text} />
    </span>
  );
}

export const TAGLINE = "Bättre samtal, ett tips i taget.";
