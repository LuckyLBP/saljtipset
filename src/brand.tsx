import { Bubbe } from "./mascot";

/**
 * Bubbe brand ("Humlan"): Bubbe the bumblebee (src/mascot.tsx) next to the wordmark in
 * Baloo 2, in honey and charcoal. The app icon (src-tauri/icons) is Bubbe on a charcoal tile.
 */

export function Wordmark({ size = 17 }: { size?: number }) {
  return (
    <span className="font-display font-extrabold tracking-[-0.02em] text-ink" style={{ fontSize: size }}>
      Bubbe
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
