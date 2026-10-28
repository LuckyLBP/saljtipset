import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { AUDIO_EXTENSIONS, errorText } from "../api";
import { Mark, TAGLINE } from "../brand";
import { Button, ErrorNote, Icon } from "../ui";

/** The empty state: drop a call (or pick a file) to start. */
export default function NewCall({
  busy,
  onFile,
  onOpenJob,
}: {
  busy: boolean;
  onFile: (path: string) => Promise<void>;
  onOpenJob: () => void;
}) {
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const startRef = useRef<(p: string) => void>(() => {});

  const start = async (path: string) => {
    setError(null);
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    if (!AUDIO_EXTENSIONS.includes(ext)) {
      setError(`Filtypen .${ext} stöds inte. Använd mp3, m4a, wav, ogg, webm, mp4 eller aac.`);
      return;
    }
    try {
      await onFile(path);
    } catch (e) {
      setError(errorText(e));
    }
  };
  startRef.current = start;

  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent((e) => {
      const t = e.payload.type;
      if (t === "enter" || t === "over") setDragging(true);
      else if (t === "leave") setDragging(false);
      else if (t === "drop") {
        setDragging(false);
        const path = e.payload.paths[0];
        if (path) startRef.current(path);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const pick = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      title: "Välj ett samtal",
      filters: [{ name: "Ljud och video", extensions: AUDIO_EXTENSIONS }],
    });
    if (typeof path === "string") start(path);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="h-[52px] shrink-0" data-tauri-drag-region />
      <div className="flex flex-1 items-center justify-center px-10 pb-16">
        <div className="w-full max-w-xl space-y-5 text-center">
          <div
            onClick={busy ? undefined : pick}
            className={`flex flex-col items-center gap-4 rounded-[28px] border-[1.5px] border-dashed px-10 py-14 transition-all ${
              dragging
                ? "scale-[1.01] border-accent bg-accent-soft"
                : "border-line bg-surface/60 hover:border-accent/50 hover:bg-surface"
            } ${busy ? "opacity-60" : ""}`}
          >
            <Mark size={56} />
            <div className="space-y-1">
              <h1 className="font-display text-[22px] font-semibold tracking-[-0.02em]">
                {busy ? "Ett samtal transkriberas just nu" : "Släpp ett samtal här"}
              </h1>
              <p className="text-[13px] text-muted">
                {busy ? "Du kan släppa nästa när det är klart." : "eller välj en fil. Ljudet stannar på din dator."}
              </p>
            </div>
            {busy ? (
              <Button onClick={onOpenJob}>Visa förloppet</Button>
            ) : (
              <Button variant="primary" size="lg" onClick={(e) => (e.stopPropagation(), pick())}>
                <Icon.Plus /> Välj fil
              </Button>
            )}
            <p className="text-[11.5px] text-faint">mp3 · m4a · wav · ogg · webm · mp4 · aac</p>
          </div>
          {error && <ErrorNote>{error}</ErrorNote>}
          <p className="text-[12px] text-faint">{TAGLINE}</p>
        </div>
      </div>
    </div>
  );
}
