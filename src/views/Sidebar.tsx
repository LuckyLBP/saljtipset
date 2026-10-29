import { useMemo, useState } from "react";
import { type CallSummary, VERDICT_LABEL } from "../api";
import type { LiveJob, View } from "../App";
import { Logo } from "../brand";
import { Icon, ProgressBar, Spinner } from "../ui";

/** Left column: brand, new call, search, the call list and settings. Glass shows through on macOS. */
export default function Sidebar({
  calls,
  view,
  job,
  queue,
  tipsRunning,
  onNavigate,
}: {
  calls: CallSummary[] | null;
  view: View;
  job: LiveJob | null;
  /** Calls waiting to be transcribed, in order. */
  queue: string[];
  tipsRunning: string[];
  onNavigate: (v: View) => void;
}) {
  const [query, setQuery] = useState("");
  const groups = useMemo(() => group((calls ?? []).filter((c) => matches(c, query))), [calls, query]);

  return (
    <aside className="sidebar flex w-[264px] shrink-0 flex-col">
      {/* Room for the traffic lights; the strip itself drags the window. */}
      <div className="h-[52px] shrink-0" data-tauri-drag-region />

      <div className="flex items-center justify-between px-4 pb-3" data-tauri-drag-region>
        <Logo mark={26} text={16} />
      </div>

      <div className="space-y-2 px-3 pb-3">
        <button
          onClick={() => onNavigate({ name: "new" })}
          className={`flex h-9 w-full items-center justify-center gap-1.5 rounded-full text-[13px] font-medium shadow-sm transition-colors ${
            view.name === "new" ? "bg-honey text-on-accent" : "bg-honey text-on-accent hover:bg-honey-strong"
          }`}
        >
          <Icon.Plus /> Nytt samtal
        </button>
        <label className="flex h-8 items-center gap-2 rounded-full bg-sunken px-3 text-muted focus-within:ring-2 focus-within:ring-accent/40">
          <Icon.Search />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Sök"
            spellCheck={false}
            className="selectable w-full bg-transparent text-[13px] text-ink outline-none placeholder:text-faint"
          />
        </label>
      </div>

      <nav className="min-h-0 flex-1 overflow-auto px-2 pb-2">
        {calls === null ? (
          <div className="py-6 text-center text-muted">
            <Spinner />
          </div>
        ) : calls.length === 0 ? (
          <p className="px-3 py-6 text-center text-[12.5px] leading-relaxed text-muted">
            Dina samtal hamnar här när du har transkriberat ett.
          </p>
        ) : groups.length === 0 ? (
          <p className="px-3 py-6 text-center text-[12.5px] text-muted">Inga samtal matchar.</p>
        ) : (
          groups.map(([label, items]) => (
            <div key={label} className="mb-2">
              <div className="px-3 pb-1 pt-2 text-[11.5px] font-semibold text-faint">{label}</div>
              {items.map((c) => {
                const active = view.name === "call" && view.id === c.id;
                const live = job?.callId === c.id;
                const place = queue.indexOf(c.id) + 1;
                return (
                  <button
                    key={c.id}
                    onClick={() => onNavigate({ name: "call", id: c.id })}
                    className={`group flex w-full items-center gap-2.5 rounded-xl px-3 py-2 text-left transition-colors ${
                      active ? "bg-accent-soft" : "hover:bg-hover"
                    }`}
                  >
                    <div className="min-w-0 flex-1">
                      <div className={`truncate text-[13px] font-medium ${active ? "text-accent" : "text-ink"}`}>{c.title}</div>
                      <div className="truncate text-[11.5px] text-muted">
                        {live
                          ? `${job.phase === "transcribing" ? "Transkriberar" : "Förbereder"} ${Math.round(job.percent)} %`
                          : place
                            ? `I kö · ${place === 1 ? "näst på tur" : `plats ${place}`}`
                            : subtitle(c)}
                      </div>
                      {(live || place > 0) && (
                        <div className="pb-0.5 pt-1.5">
                          <ProgressBar
                            value={live ? job.percent : 0}
                            indeterminate={live && job.phase === "loading"}
                          />
                        </div>
                      )}
                    </div>
                    <StatusDot call={c} live={live} queued={place > 0} tips={tipsRunning.includes(c.id)} />
                  </button>
                );
              })}
            </div>
          ))
        )}
      </nav>

      <div className="px-3 pb-3 pt-1">
        <button
          onClick={() => onNavigate({ name: "settings" })}
          className={`flex h-9 w-full items-center gap-2 rounded-full px-3 text-[13px] transition-colors ${
            view.name === "settings" ? "bg-accent-soft font-medium text-accent" : "text-muted hover:bg-hover hover:text-ink"
          }`}
        >
          <Icon.Gear /> Inställningar
        </button>
      </div>
    </aside>
  );
}

function StatusDot({ call, live, queued, tips }: { call: CallSummary; live: boolean; queued: boolean; tips: boolean }) {
  if (live || call.status === "transcribing" || tips) return <Spinner className="text-accent" />;
  if (queued) return <span className="h-2 w-2 rounded-full border-[1.5px] border-faint" title="I kö" />;
  if (call.status === "error") return <span className="h-2 w-2 rounded-full bg-danger" title="Misslyckades" />;
  if (call.verdict) {
    const dot = { godkant: "bg-ok", bor_ses_over: "bg-warn", allvarligt: "bg-danger" }[call.verdict];
    return <span className={`h-2.5 w-2.5 rounded-full ${dot}`} title={`Granskning: ${VERDICT_LABEL[call.verdict]}`} />;
  }
  if (call.hasTips)
    return (
      <span className="text-accent" title="Säljtips klara">
        <Icon.Sparkle />
      </span>
    );
  return null;
}

function subtitle(c: CallSummary): string {
  const d = new Date(c.createdAt);
  const time = d.toLocaleTimeString("sv-SE", { hour: "2-digit", minute: "2-digit" });
  const mins = c.durationMs > 0 ? ` · ${Math.max(1, Math.round(c.durationMs / 60000))} min` : "";
  if (c.status === "error") return "Misslyckades";
  return isToday(d) ? `${time}${mins}` : `${d.toLocaleDateString("sv-SE", { day: "numeric", month: "short" })}${mins}`;
}

function isToday(d: Date) {
  return d.toDateString() === new Date().toDateString();
}

function matches(c: CallSummary, q: string) {
  const s = q.trim().toLowerCase();
  return !s || c.title.toLowerCase().includes(s) || c.fileName.toLowerCase().includes(s);
}

/** Group by recency like Mail/Notes: Idag, Igår, Den här veckan, then month. */
function group(calls: CallSummary[]): [string, CallSummary[]][] {
  const out = new Map<string, CallSummary[]>();
  const now = new Date();
  const day = 86400000;
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  for (const c of calls) {
    const t = new Date(c.createdAt).getTime();
    let label: string;
    if (t >= startOfToday) label = "Idag";
    else if (t >= startOfToday - day) label = "Igår";
    else if (t >= startOfToday - 6 * day) label = "Den här veckan";
    else label = capitalize(new Date(t).toLocaleDateString("sv-SE", { month: "long", year: "numeric" }));
    out.set(label, [...(out.get(label) ?? []), c]);
  }
  return [...out.entries()];
}

function capitalize(s: string) {
  return s.charAt(0).toUpperCase() + s.slice(1);
}
