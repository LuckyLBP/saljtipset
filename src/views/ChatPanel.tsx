import { type ReactNode, useEffect, useRef, useState } from "react";
import { api, type ChatMessage, errorText, type Provider, PROVIDER_VENDOR } from "../api";
import { Mark } from "../brand";
import { Button, ErrorNote, Icon, IconButton } from "../ui";

const SUGGESTIONS = [
  "Hur var mitt samtal?",
  "Vad kunde jag ha sagt istället?",
  "Hur bemöter jag kundens invändningar?",
  "Vilka frågor borde jag ha ställt?",
];

/** Small chat window for asking the AI about one call. History is saved per call. */
export default function ChatPanel({
  callId,
  provider,
  onJumpToTime,
  onOpenSettings,
}: {
  callId: string;
  provider: Provider | null;
  onJumpToTime: (ms: number) => void;
  onOpenSettings: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    api.chatHistory(callId).then(setMessages).catch(() => {});
  }, [callId]);

  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages, pending, open]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  const send = async (text: string) => {
    const q = text.trim();
    if (!q || pending) return;
    setError(null);
    setPending(q);
    setDraft("");
    try {
      setMessages(await api.sendChat(callId, q));
    } catch (e) {
      setError(errorText(e));
      setDraft(q); // give the question back so it can be retried
    } finally {
      setPending(null);
    }
  };

  const clear = async () => {
    await api.clearChat(callId);
    setMessages([]);
    setError(null);
  };

  if (!open) {
    return (
      <button
        onClick={() => setOpen(true)}
        className="glass rise absolute bottom-5 right-5 z-20 flex h-11 items-center gap-2 rounded-full pl-4 pr-5 text-[13px] font-medium text-ink transition-transform hover:scale-[1.02]"
      >
        <span className="text-accent">
          <Icon.Chat />
        </span>
        Fråga om samtalet
        {messages.length > 0 && <span className="rounded-full bg-accent-soft px-1.5 text-[11px] text-accent">{messages.length / 2}</span>}
      </button>
    );
  }

  return (
    <div className="glass rise absolute bottom-5 right-5 z-20 flex h-[min(600px,calc(100%-40px))] w-[400px] flex-col overflow-hidden rounded-[24px]">
      <div className="flex items-center gap-2.5 px-4 pb-2 pt-3">
        <Mark size={24} />
        <div className="min-w-0 flex-1">
          <div className="text-[13.5px] font-semibold leading-tight">Fråga om samtalet</div>
          {provider && <div className="text-[11px] text-muted">Texten skickas till {PROVIDER_VENDOR[provider]}</div>}
        </div>
        {messages.length > 0 && (
          <IconButton label="Rensa chatten" onClick={clear} disabled={pending !== null}>
            <Icon.Refresh />
          </IconButton>
        )}
        <IconButton label="Stäng" onClick={() => setOpen(false)}>
          <Icon.Close />
        </IconButton>
      </div>

      <div ref={listRef} className="min-h-0 flex-1 space-y-2.5 overflow-auto px-4 py-2">
        {messages.length === 0 && !pending && (
          <div className="space-y-3 pt-6 text-center">
            <p className="mx-auto max-w-[280px] text-[12.5px] leading-relaxed text-muted">
              Fråga vad som helst om samtalet. AI:n har hela transkriberingen och säljtipsen framför sig.
            </p>
            <div className="flex flex-wrap justify-center gap-1.5">
              {SUGGESTIONS.map((s) => (
                <button
                  key={s}
                  onClick={() => send(s)}
                  disabled={!provider}
                  className="rounded-full bg-surface/80 px-3 py-1.5 text-[12px] shadow-[0_0_0_0.5px_var(--color-line)] transition-colors hover:bg-accent-soft hover:text-accent disabled:opacity-50"
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
        )}
        {messages.map((m) => (
          <Bubble key={m.id} role={m.role}>
            {m.role === "assistant" ? <Answer text={m.text} onJumpToTime={onJumpToTime} /> : m.text}
          </Bubble>
        ))}
        {pending && (
          <>
            <Bubble role="user">{pending}</Bubble>
            <Bubble role="assistant">
              <span className="typing inline-flex gap-1 py-1 text-muted" aria-label="Skriver">
                <span>●</span>
                <span>●</span>
                <span>●</span>
              </span>
            </Bubble>
          </>
        )}
        {error && <ErrorNote>{error}</ErrorNote>}
      </div>

      <div className="p-3 pt-2">
        {provider ? (
          <div className="flex items-end gap-2 rounded-[20px] bg-surface/90 p-1.5 pl-3.5 shadow-[0_0_0_0.5px_var(--color-line)] focus-within:shadow-[0_0_0_2px_var(--color-mark)]">
            <textarea
              ref={inputRef}
              rows={1}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  send(draft);
                }
              }}
              placeholder="Skulle jag kunnat säga något annat?"
              className="selectable max-h-28 min-h-[30px] flex-1 resize-none bg-transparent py-1.5 text-[13px] outline-none placeholder:text-faint"
            />
            <button
              onClick={() => send(draft)}
              disabled={!draft.trim() || pending !== null}
              aria-label="Skicka"
              className="flex h-[30px] w-[30px] shrink-0 items-center justify-center rounded-full bg-accent text-on-accent transition-opacity disabled:opacity-30"
            >
              <Icon.Send />
            </button>
          </div>
        ) : (
          <div className="flex items-center justify-between gap-2 px-1 text-[12.5px] text-muted">
            Ingen AI är kopplad än.
            <Button size="sm" onClick={onOpenSettings}>
              Inställningar
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

function Bubble({ role, children }: { role: "user" | "assistant"; children: ReactNode }) {
  return role === "user" ? (
    <div className="rise flex justify-end">
      <div className="selectable max-w-[85%] rounded-[18px] rounded-br-[6px] bg-accent px-3.5 py-2 text-[13px] leading-relaxed text-on-accent">
        {children}
      </div>
    </div>
  ) : (
    <div className="rise flex justify-start">
      <div className="selectable max-w-[92%] rounded-[18px] rounded-bl-[6px] bg-surface px-3.5 py-2.5 text-[13px] leading-relaxed shadow-[0_0_0_0.5px_var(--color-line)]">
        {children}
      </div>
    </div>
  );
}

/**
 * Minimal Markdown for answers: paragraphs, "-"/"1." lists, **bold**, *italic*, and
 * [mm:ss] timestamps that jump to the line in the transcript.
 */
function Answer({ text, onJumpToTime }: { text: string; onJumpToTime: (ms: number) => void }) {
  const blocks = text
    .trim()
    .replace(/^#{1,4}\s+(.+)$/gm, "**$1**")
    .split(/\n{2,}/);
  return (
    <div className="space-y-2">
      {blocks.map((block, i) => {
        const lines = block.split("\n");
        const isList = lines.every((l) => /^\s*([-*•]|\d+[.)])\s+/.test(l) || /^\s{2,}\S/.test(l));
        if (isList) {
          const ordered = /^\s*\d+[.)]/.test(lines[0]);
          const items: string[] = [];
          for (const l of lines) {
            if (/^\s*([-*•]|\d+[.)])\s+/.test(l)) items.push(l.replace(/^\s*([-*•]|\d+[.)])\s+/, ""));
            else items[items.length - 1] += "\n" + l.trim();
          }
          const ListTag = ordered ? "ol" : "ul";
          return (
            <ListTag key={i} className={`space-y-1 pl-5 ${ordered ? "list-decimal" : "list-disc"}`}>
              {items.map((it, j) => (
                <li key={j} className="whitespace-pre-line">
                  <Inline text={it} onJumpToTime={onJumpToTime} />
                </li>
              ))}
            </ListTag>
          );
        }
        return (
          <p key={i} className="whitespace-pre-line">
            <Inline text={block} onJumpToTime={onJumpToTime} />
          </p>
        );
      })}
    </div>
  );
}

function Inline({ text, onJumpToTime }: { text: string; onJumpToTime: (ms: number) => void }) {
  const parts = text.split(/(\*\*[^*]+\*\*|\*[^*\n]+\*|\[\d{1,3}:\d{2}\])/g);
  return (
    <>
      {parts.map((p, i) => {
        if (/^\*\*[^*]+\*\*$/.test(p))
          return (
            <strong key={i}>
              <Inline text={p.slice(2, -2)} onJumpToTime={onJumpToTime} />
            </strong>
          );
        if (/^\*[^*]+\*$/.test(p)) return <em key={i}>{p.slice(1, -1)}</em>;
        const ts = /^\[(\d{1,3}):(\d{2})\]$/.exec(p);
        if (ts) {
          const ms = (Number(ts[1]) * 60 + Number(ts[2])) * 1000;
          return (
            <button
              key={i}
              onClick={() => onJumpToTime(ms)}
              className="rounded bg-sunken px-1 font-mono text-[11.5px] text-accent hover:bg-accent-soft"
              title="Visa i samtalet"
            >
              {p}
            </button>
          );
        }
        return <span key={i}>{p}</span>;
      })}
    </>
  );
}
