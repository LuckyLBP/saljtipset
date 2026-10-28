import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { AUDIO_EXTENSIONS, errorText } from "../api";
import { TAGLINE } from "../brand";
import { Bubbe } from "../mascot";
import { Button, ErrorNote, Icon } from "../ui";

/**
 * The empty state: drop calls (or pick files) to start. Several at once is fine, and more can
 * be added while one runs: they wait in the queue and are transcribed one after the other.
 */
export default function NewCall({
  busy,
  queued,
  onFiles,
  onOpenJob,
}: {
  busy: boolean;
  /** How many calls are waiting behind the one being transcribed. */
  queued: number;
  /** Resolves with how many calls were queued. */
  onFiles: (paths: string[]) => Promise<number>;
  onOpenJob: () => void;
}) {
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [added, setAdded] = useState<number | null>(null);
  // Bubbe says hello when the screen appears and dozes off after a while without activity.
  const [greeting, setGreeting] = useState(true);
  const [asleep, setAsleep] = useState(false);
  useEffect(() => {
    const hello = setTimeout(() => setGreeting(false), 2600);
    let nap = setTimeout(() => setAsleep(true), 90_000);
    const wake = () => {
      setAsleep(false);
      clearTimeout(nap);
      nap = setTimeout(() => setAsleep(true), 90_000);
    };
    window.addEventListener("mousemove", wake);
    window.addEventListener("keydown", wake);
    return () => {
      clearTimeout(hello);
      clearTimeout(nap);
      window.removeEventListener("mousemove", wake);
      window.removeEventListener("keydown", wake);
    };
  }, []);
  const mood = busy ? "listening" : dragging ? "surprised" : error ? "oops" : greeting ? "hello" : asleep ? "sleepy" : "idle";
  const startRef = useRef<(p: string[]) => void>(() => {});

  const start = async (paths: string[]) => {
    setError(null);
    setAdded(null);
    const ext = (p: string) => p.split(".").pop()?.toLowerCase() ?? "";
    const ok = paths.filter((p) => AUDIO_EXTENSIONS.includes(ext(p)));
    const skipped = paths.filter((p) => !AUDIO_EXTENSIONS.includes(ext(p)));
    const skippedNote = skipped.length
      ? `${skipped.length === 1 ? `Filtypen .${ext(skipped[0])} stöds inte` : `${skipped.length} filer hoppades över, filtypen stöds inte`}. Använd mp3, m4a, wav, ogg, webm, mp4 eller aac.`
      : null;
    if (ok.length === 0) {
      setError(skippedNote);
      return;
    }
    try {
      const n = await onFiles(ok);
      setAdded(n);
      setError(skippedNote);
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
        if (e.payload.paths.length) startRef.current(e.payload.paths);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const pick = async () => {
    const paths = await open({
      multiple: true,
      directory: false,
      title: "Välj samtal",
      filters: [{ name: "Ljud och video", extensions: AUDIO_EXTENSIONS }],
    });
    if (paths?.length) start(paths);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="h-[52px] shrink-0" data-tauri-drag-region />
      <div className="flex flex-1 items-center justify-center px-10 pb-16">
        <div className="w-full max-w-xl space-y-5 text-center">
          <div
            onClick={pick}
            className={`flex flex-col items-center gap-4 rounded-[28px] border-[1.5px] border-dashed px-10 py-14 transition-all ${
              dragging
                ? "scale-[1.01] border-accent bg-accent-soft"
                : "border-line bg-surface/60 hover:border-accent/50 hover:bg-surface"
            }`}
          >
            <Bubbe mood={mood} size={dragging ? 118 : 104} interactive className="transition-all" />
            <div className="space-y-1">
              <h1 className="font-display text-[22px] font-semibold tracking-[-0.02em]">
                {busy ? "Bubbe lyssnar på ett samtal" : "Släpp samtal här"}
              </h1>
              <p className="text-[13px] text-muted">
                {busy
                  ? `Släpp fler så ställs de i kön${queued ? ` (${queued} väntar)` : ""}.`
                  : "Ett eller flera på en gång. Ljudet stannar på din dator."}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="primary" size="lg" onClick={(e) => (e.stopPropagation(), pick())}>
                <Icon.Plus /> Välj filer
              </Button>
              {busy && (
                <Button size="lg" onClick={(e) => (e.stopPropagation(), onOpenJob())}>
                  Visa förloppet
                </Button>
              )}
            </div>
            <p className="text-[11.5px] text-faint">mp3 · m4a · wav · ogg · webm · mp4 · aac</p>
          </div>
          {added !== null && busy && (
            <p className="text-[13px] font-medium text-accent">
              {added === 1 ? "Samtalet ställdes i kön." : `${added} samtal ställdes i kön.`}
            </p>
          )}
          {error && <ErrorNote>{error}</ErrorNote>}
          <p className="text-[12px] text-faint">{TAGLINE}</p>
        </div>
      </div>
    </div>
  );
}
