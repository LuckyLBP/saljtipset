import { Bubbe } from "./mascot";

/**
 * Säljtipset brand: Bubbe the mascot (src/mascot.tsx) next to the wordmark, in
 * "tipset-grön". The app icon (src-tauri/icons) is Bubbe on a green glass tile.
 */

export function Wordmark({ size = 17 }: { size?: number }) {
  return (
    <span className="font-display font-semibold tracking-[-0.02em] text-ink" style={{ fontSize: size }}>
      Säljtipset
    </span>
  );
}

export function Logo({ mark = 26, text = 16 }: { mark?: number; text?: number }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <Bubbe size={mark + 8} />
      <Wordmark size={text} />
    </span>
  );
}

export const TAGLINE = "Bättre samtal, ett tips i taget.";
