import { useEffect, useMemo, useRef, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ask, save } from "@tauri-apps/plugin-dialog";
import {
  api,
  type AppInfo,
  type Call,
  type CallStats,
  errorText,
  type ExportFormat,
  formatDate,
  formatDuration,
  formatTimestamp,
  type Segment,
  type Tips,
} from "../api";
import type { LiveJob, TipsState } from "../App";
import { Button, ConfirmButton, ErrorNote, Icon, IconButton, Menu, Pill, ProgressBar, Segmented, Spinner } from "../ui";
import ChatPanel from "./ChatPanel";
import { Bubbe } from "../mascot";

const PHASE_LABEL = {
  decoding: "Läser in ljudet",
  loading: "Laddar talmodellen",
  transcribing: "Transkriberar",
} as const;

export default function CallView({
  id,
  info,
  revision,
  job,
  jobError,
  tipsState,
  onTipsState,
  onDeleted,
  onChanged,
  onOpenSettings,
}: {
  id: string;
  info: AppInfo;
  revision: number;
  job: LiveJob | null;
  jobError: string | null;
  tipsState?: TipsState[string];
  onTipsState: (s: TipsState[string]) => void;
  onDeleted: () => void;
  onChanged: () => void;
  onOpenSettings: () => void;
}) {
  const [call, setCall] = useState<Call | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [highlight, setHighlight] = useState<{ id: number; n: number } | null>(null);
  const [tab, setTab] = useState<"overview" | "transcript">(job ? "transcript" : "overview");
  const [editingTitle, setEditingTitle] = useState(false);
  const lineRefs = useRef(new Map<number, HTMLDivElement>());
  const transcriptRef = useRef<HTMLDivElement>(null);
  const followLive = useRef(true);

  useEffect(() => {
    api.getCall(id).then(setCall).catch((e) => setError(errorText(e)));
  }, [id, revision]);

  const transcribing = call?.status === "transcribing" || job !== null;
  const segments: Segment[] = useMemo(() => {
    if (job && job.segments.length > 0) return [...job.segments].sort((a, b) => a.startMs - b.startMs);
    return call?.segments ?? [];
  }, [job, call]);
  const tips: Tips | null = tipsState?.tips ?? call?.tips ?? null;

  // Name and role per line, computed by the backend once the AI knows who's who.
  // While transcribing (provisional ids) only the channel speaker is known.
  const labelOf = (s: Segment) => (job ? s.speaker : (call?.labels[s.id] ?? s.speaker)) ?? null;
  const roleOf = (s: Segment) => (job ? null : (call?.roles[s.id] ?? null));

  // Keep the live transcript scrolled to the bottom unless the user scrolled up.
  useEffect(() => {
    const el = transcriptRef.current;
    if (job && el && followLive.current) el.scrollTop = el.scrollHeight;
  }, [job, segments.length]);

  const jumpTo = (segmentId: number | null) => {
    if (segmentId === null) return;
    setTab("transcript");
    setHighlight((h) => ({ id: segmentId, n: (h?.n ?? 0) + 1 }));
  };

  // Scroll once the transcript tab is on screen.
  useEffect(() => {
    if (!highlight) return;
    requestAnimationFrame(() =>
      lineRefs.current.get(highlight.id)?.scrollIntoView({ behavior: "smooth", block: "center" }),
    );
  }, [highlight]);

  // A chat answer's [mm:ss]: the last line that starts at or before that time.
  const jumpToTime = (ms: number) => {
    const target = [...segments].reverse().find((s) => s.startMs <= ms + 999) ?? segments[0];
    if (target) jumpTo(target.id);
  };

  const runTips = async () => {
    onTipsState({ running: true });
    try {
      const t = await api.generateTips(id);
      onTipsState({ running: false, tips: t });
      onChanged();
    } catch (e) {
      onTipsState({ running: false, error: errorText(e) });
    }
  };

  const rename = async (title: string) => {
    try {
      await api.renameCall(id, title);
      setCall((c) => (c ? { ...c, title } : c));
      onChanged();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const remove = async () => {
    const ok = await ask(`"${call?.title}" tas bort från Bubbe. Ljudfilen påverkas inte.`, {
      title: "Ta bort samtalet?",
      kind: "warning",
      okLabel: "Ta bort",
      cancelLabel: "Avbryt",
    });
    if (!ok) return;
    await api.deleteCall(id);
    onDeleted();
  };

  const exportAs = async (format: ExportFormat) => {
    if (!call) return;
    const names = { txt: "Textfil", md: "Markdown", docx: "Word-dokument" };
    const path = await save({
      title: "Exportera samtalet",
      defaultPath: `${call.title.replace(/[\\/:*?"<>|]/g, "-")}.${format}`,
      filters: [{ name: names[format], extensions: [format] }],
    });
    if (!path) return;
    try {
      await api.exportCall(id, format, path);
    } catch (e) {
      setError(errorText(e));
    }
  };

  if (!call) {
    return (
      <div className="flex h-full items-center justify-center text-muted" data-tauri-drag-region>
        {error ? <ErrorNote>{error}</ErrorNote> : <Spinner />}
      </div>
    );
  }

  const failed = call.status === "error" && !job;
  const stereo = call.channels > 1;
  const customer = [tips?.customerName, tips?.customerCompany].filter(Boolean).join(", ");
  const guessedRoles = !stereo && (call.roles ?? []).some(Boolean);
  const hasAi = info.activeProvider !== null;

  return (
    <div className="flex h-full flex-col">
      {/* Toolbar. The bar itself drags the window. */}
      <header className="flex h-[60px] shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <div className="min-w-0 flex-1" data-tauri-drag-region>
          <EditableTitle value={call.title} editing={editingTitle} setEditing={setEditingTitle} onSave={rename} />
          <div className="truncate text-[11.5px] text-muted" data-tauri-drag-region>
            {formatDate(call.createdAt)}
            {call.durationMs > 0 && ` · ${formatDuration(call.durationMs)}`}
            {customer ? ` · med ${customer}` : stereo ? " · två talare" : ""}
          </div>
        </div>

        <Segmented
          value={tab}
          onChange={setTab}
          options={[
            { value: "overview", label: "Översikt" },
            { value: "transcript", label: "Transkribering" },
          ]}
        />

        <div className="flex flex-1 items-center justify-end gap-1.5">
          {tab === "transcript" ? (
            <ConfirmButton
              label="Kopiera transkribering"
              done="Kopierat!"
              icon={<Icon.Copy />}
              disabled={transcribing || segments.length === 0}
              onAction={async () => writeText(await api.transcriptText(id))}
            />
          ) : (
            <ConfirmButton
              label="Kopiera säljtips"
              done="Kopierat!"
              icon={<Icon.Copy />}
              disabled={!tips}
              onAction={async () => writeText(await api.tipsText(id))}
            />
          )}
          <Menu
            trigger={(toggle) => (
              <IconButton label="Exportera" onClick={toggle} disabled={transcribing || segments.length === 0}>
                <Icon.Share />
              </IconButton>
            )}
            items={[
              { label: "Word-dokument (.docx)", onSelect: () => exportAs("docx") },
              { label: "Textfil (.txt)", onSelect: () => exportAs("txt") },
              { label: "Markdown (.md)", onSelect: () => exportAs("md") },
            ]}
          />
          <Menu
            trigger={(toggle) => (
              <IconButton label="Mer" onClick={toggle}>
                <Icon.More />
              </IconButton>
            )}
            items={[
              { label: "Byt namn", icon: <Icon.Pencil />, onSelect: () => setEditingTitle(true) },
              {
                label: tips ? "Ta fram säljtips igen" : "Ta fram säljtips",
                icon: <Icon.Sparkle />,
                disabled: !hasAi || transcribing || segments.length === 0 || tipsState?.running,
                onSelect: () => {
                  setTab("overview");
                  runTips();
                },
              },
              "divider",
              { label: "Ta bort samtalet", icon: <Icon.Trash />, danger: true, onSelect: remove },
            ]}
          />
        </div>
      </header>

      {transcribing && job && (
        <div className="flex items-center gap-3 px-5 pb-3">
          {/* Bubbe nods along and hops each time a new line arrives. */}
          <span key={segments.length} className="bubbe-hop -my-2 inline-flex">
            <Bubbe mood={job.phase === "transcribing" ? "listening" : "thinking"} size={40} />
          </span>
          <div className="flex-1">
            <ProgressBar value={job.phase === "loading" ? 0 : job.percent} indeterminate={job.phase === "loading"} />
          </div>
          <span className="text-[12px] tabular-nums text-muted">
            {PHASE_LABEL[job.phase]}
            {job.phase !== "loading" && ` · ${Math.round(job.percent)} %`}
          </span>
          <Button size="sm" onClick={() => api.cancelTranscription()}>
            Avbryt
          </Button>
        </div>
      )}
      {(failed || jobError || error) && (
        <div className="flex items-center gap-3 px-5 pb-3">
          <Bubbe mood="oops" size={44} />
          <div className="flex-1">
            <ErrorNote>{error ?? jobError ?? call.error ?? "Transkriberingen misslyckades."}</ErrorNote>
          </div>
        </div>
      )}

      <div className="relative min-h-0 flex-1 border-t border-line">
        {tab === "overview" ? (
          <div className="h-full overflow-auto">
            <div className="mx-auto max-w-[720px] px-8 pb-28 pt-7">
              <Overview
                tips={tips}
                state={tipsState}
                transcribing={transcribing}
                hasSegments={segments.length > 0}
                hasAi={hasAi}
                stats={call.stats}
                onRun={runTips}
                onJump={jumpTo}
                onOpenSettings={onOpenSettings}
              />
            </div>
          </div>
        ) : (
          <section
            ref={transcriptRef}
            onScroll={(e) => {
              const el = e.currentTarget;
              followLive.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
            }}
            className="h-full overflow-auto"
          >
            <div className="mx-auto max-w-[760px] px-8 pb-28 pt-6">
              {guessedRoles && <p className="mb-4 text-[12px] text-muted">Vem som säger vad är gissat av AI:n utifrån innehållet.</p>}
              {segments.length === 0 ? (
                <p className="py-16 text-center text-[13px] text-muted">
                  {transcribing ? "Texten dyker upp här allt eftersom." : "Ingen text hittades i ljudfilen."}
                </p>
              ) : (
                <div className="selectable">
                  {segments.map((s, i) => {
                    const label = labelOf(s);
                    const role = roleOf(s);
                    const prev = i > 0 ? labelOf(segments[i - 1]) : undefined;
                    const newSpeaker = label !== null && label !== prev;
                    const second = role ? role === "Kund" : label === "Talare 2";
                    const isHl = highlight?.id === s.id && !job;
                    return (
                      <div
                        key={`${s.id}-${i}-${isHl ? highlight?.n : 0}`}
                        ref={(el) => {
                          if (el && !job) lineRefs.current.set(s.id, el);
                        }}
                        className={`group -mx-3 grid grid-cols-[52px_1fr] gap-3 rounded-xl px-3 py-1.5 ${newSpeaker && i > 0 ? "mt-3" : ""} ${
                          isHl ? "flash" : ""
                        }`}
                      >
                        <span className="pt-[3px] text-right font-mono text-[11.5px] tabular-nums text-faint">
                          {formatTimestamp(s.startMs)}
                        </span>
                        <div className="text-[14px] leading-[1.6]">
                          {newSpeaker && (
                            <div className={`text-[12px] font-semibold ${second ? "text-speaker-b" : "text-speaker-a"}`}>
                              {label}
                              {role === "Säljare" && <span className="ml-1.5 font-normal text-faint">du</span>}
                            </div>
                          )}
                          {s.text}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </section>
        )}

        {!transcribing && segments.length > 0 && (
          <ChatPanel callId={id} provider={info.activeProvider} onJumpToTime={jumpToTime} onOpenSettings={onOpenSettings} />
        )}
      </div>
    </div>
  );
}

function EditableTitle({
  value,
  editing,
  setEditing,
  onSave,
}: {
  value: string;
  editing: boolean;
  setEditing: (v: boolean) => void;
  onSave: (v: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value, editing]);
  if (!editing) {
    return (
      <button
        onDoubleClick={() => setEditing(true)}
        title="Dubbelklicka för att byta namn"
        className="block max-w-full truncate text-left font-display text-[15.5px] font-semibold tracking-[-0.015em]"
      >
        {value}
      </button>
    );
  }
  const commit = () => {
    setEditing(false);
    const v = draft.trim();
    if (v && v !== value) onSave(v);
  };
  return (
    <input
      autoFocus
      value={draft}
      onFocus={(e) => e.target.select()}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
        if (e.key === "Escape") setEditing(false);
      }}
      className="selectable -ml-1.5 w-full max-w-sm rounded-lg bg-surface px-1.5 font-display text-[15.5px] font-semibold tracking-[-0.015em] outline-none ring-2 ring-accent/50"
    />
  );
}

function Overview({
  tips,
  state,
  transcribing,
  hasSegments,
  hasAi,
  stats,
  onRun,
  onJump,
  onOpenSettings,
}: {
  tips: Tips | null;
  state?: TipsState[string];
  transcribing: boolean;
  hasSegments: boolean;
  hasAi: boolean;
  stats: CallStats | null;
  onRun: () => void;
  onJump: (segmentId: number | null) => void;
  onOpenSettings: () => void;
}) {
  if (state?.running) {
    return (
      <Empty icon={<Bubbe mood="thinking" size={96} />} title="Tar fram säljtips…">
        AI:n läser samtalet. Bara texten skickas, aldrig ljudet.
      </Empty>
    );
  }
  if (!tips) {
    return (
      <Empty
        icon={<Bubbe mood={transcribing ? "listening" : state?.error ? "oops" : "idle"} size={96} interactive={!transcribing} />}
        title={transcribing ? "Säljtipsen kommer sen" : "Inga säljtips än"}
        action={
          transcribing ? null : !hasAi ? (
            <Button onClick={onOpenSettings}>Koppla en AI</Button>
          ) : (
            <Button variant="primary" size="lg" onClick={onRun} disabled={!hasSegments}>
              <Icon.Sparkle /> Ta fram säljtips
            </Button>
          )
        }
      >
        {transcribing
          ? "När transkriberingen är klar kan AI:n visa vad som fungerade och vad du kan göra bättre."
          : !hasAi
            ? "Använd din Claude- eller ChatGPT-prenumeration, eller en Anthropic API-nyckel."
            : "AI:n läser samtalet och visar vad som fungerade, kundens behov och vad du kan göra bättre."}
        {state?.error && (
          <div className="mt-4 text-left">
            <ErrorNote>{state.error}</ErrorNote>
          </div>
        )}
      </Empty>
    );
  }

  return (
    <div className="space-y-8">
      <section className="relative">
        {/* Tips that just arrived get a little celebration. */}
        {state?.tips && <Bubbe mood="cheer" size={56} className="absolute -top-4 right-0" />}
        <h2 className="mb-2 text-[13px] font-semibold text-muted">Sammanfattning</h2>
        <p className="selectable font-display text-[17px] leading-[1.55] tracking-[-0.01em]">{tips.summary}</p>
        {(tips.customerName || tips.customerCompany) && (
          <div className="mt-3 flex gap-1.5">
            <Pill tone="accent">Kund: {[tips.customerName, tips.customerCompany].filter(Boolean).join(", ")}</Pill>
          </div>
        )}
      </section>

      {stats && <StatsRow stats={stats} />}

      <Section title="Det här fungerade" count={tips.sellingPoints.length} tone="accent">
        {tips.sellingPoints.map((p, i) => (
          <TipCard key={i} title={p.title} quote={p.quote} timestamp={p.timestamp} onQuote={() => onJump(p.segmentId)}>
            {p.explanation}
          </TipCard>
        ))}
      </Section>

      <Section title="Kundens behov" count={tips.customerNeeds.length}>
        {tips.customerNeeds.map((p, i) => (
          <TipCard key={i} title={p.need} quote={p.quote} timestamp={p.timestamp} onQuote={() => onJump(p.segmentId)} />
        ))}
      </Section>

      <Section title="Invändningar" count={tips.objections.length}>
        {tips.objections.map((p, i) => (
          <TipCard key={i} title={p.objection} quote={p.quote} timestamp={p.timestamp} onQuote={() => onJump(p.segmentId)}>
            <span className="font-medium text-ink">Förslag:</span> {p.suggestedAnswer}
          </TipCard>
        ))}
      </Section>

      <Section title="Missade möjligheter" count={tips.missedOpportunities.length}>
        {tips.missedOpportunities.map((p, i) => (
          <TipCard key={i} title={p.description}>
            <span className="font-medium text-ink">Förslag:</span> {p.suggestion}
          </TipCard>
        ))}
      </Section>

      <Section title="Nästa steg" count={tips.nextSteps.length}>
        <div className="rounded-2xl bg-surface p-1.5 shadow-[0_0_0_0.5px_var(--color-line),0_1px_3px_rgba(0,0,0,0.04)]">
          {tips.nextSteps.map((s, i) => (
            <div key={i} className="selectable flex items-start gap-3 rounded-xl px-3 py-2 text-[13.5px]">
              <span className="mt-[3px] flex h-4 w-4 shrink-0 items-center justify-center rounded-full border-[1.5px] border-accent/60" />
              {s}
            </div>
          ))}
        </div>
      </Section>

      <p className="text-[12px] text-faint">
        {tips.droppedPoints > 0 &&
          `${tips.droppedPoints === 1 ? "En punkt togs bort" : `${tips.droppedPoints} punkter togs bort`} eftersom citatet inte fanns i samtalet. `}
        Klicka på ett citat för att se det i transkriberingen.
      </p>
      {state?.error && <ErrorNote>{state.error}</ErrorNote>}
    </div>
  );
}

function StatsRow({ stats }: { stats: CallStats }) {
  const you = Math.round(stats.sellerTalkShare * 100);
  const secs = Math.round(stats.longestMonologueMs / 1000);
  const tiles = [
    { value: `${you} %`, label: "Du pratade", hint: `Kunden ${100 - you} %` },
    { value: String(stats.sellerQuestions), label: stats.sellerQuestions === 1 ? "Fråga från dig" : "Frågor från dig", hint: `Kunden ställde ${stats.customerQuestions}` },
    { value: `${Math.floor(secs / 60)}:${String(secs % 60).padStart(2, "0")}`, label: "Längsta monolog", hint: "Din längsta utan avbrott" },
  ];
  return (
    <section className="space-y-2">
      <div className="grid grid-cols-3 gap-2">
        {tiles.map((t) => (
          <div key={t.label} className="rounded-2xl bg-surface px-4 py-3 shadow-[0_0_0_0.5px_var(--color-line),0_1px_3px_rgba(0,0,0,0.04)]">
            <div className="font-display text-[22px] font-semibold tabular-nums tracking-[-0.02em]">{t.value}</div>
            <div className="text-[12.5px] font-medium">{t.label}</div>
            <div className="text-[11.5px] text-muted">{t.hint}</div>
          </div>
        ))}
      </div>
      <div className="flex items-center gap-2">
        <Bubbe mood={you < 50 ? "love" : "idle"} size={36} />
        <p className="text-[11.5px] text-faint">
          {you < 50 ? "Snyggt, kunden fick prata mest. " : "Ett vanligt riktmärke är att kunden pratar mer än du. "}
          Räknat från transkriberingen{stats.exact ? "" : " med gissade talare"}.
        </p>
      </div>
    </section>
  );
}

function Empty({ icon, title, action, children }: { icon: React.ReactNode; title: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-3 py-20 text-center">
      {icon}
      <h2 className="font-display text-[17px] font-semibold tracking-[-0.01em]">{title}</h2>
      <div className="max-w-sm text-[13px] leading-relaxed text-muted">{children}</div>
      {action && <div className="pt-2">{action}</div>}
    </div>
  );
}

function Section({
  title,
  count,
  tone,
  children,
}: {
  title: string;
  count?: number;
  tone?: "accent";
  children: React.ReactNode;
}) {
  if (count === 0) return null;
  return (
    <section className="space-y-2.5">
      <h2 className={`flex items-center gap-2 text-[13px] font-semibold ${tone === "accent" ? "text-accent" : "text-muted"}`}>
        {title}
        {count !== undefined && <span className="text-faint">{count}</span>}
      </h2>
      <div className="space-y-2">{children}</div>
    </section>
  );
}

function TipCard({
  title,
  quote,
  timestamp,
  onQuote,
  children,
}: {
  title: string;
  quote?: string;
  timestamp?: string;
  onQuote?: () => void;
  children?: React.ReactNode;
}) {
  return (
    <div className="space-y-2 rounded-2xl bg-surface px-4 py-3.5 shadow-[0_0_0_0.5px_var(--color-line),0_1px_3px_rgba(0,0,0,0.04)]">
      <div className="selectable text-[14px] font-semibold leading-snug">{title}</div>
      {children && <div className="selectable text-[13px] leading-relaxed text-muted">{children}</div>}
      {quote && (
        <button
          onClick={onQuote}
          className="flex max-w-full items-start gap-2 rounded-full bg-sunken py-1 pl-1 pr-3 text-left text-[12.5px] transition-colors hover:bg-accent-soft"
          title="Visa i transkriberingen"
        >
          <span className="shrink-0 rounded-full bg-surface px-2 py-0.5 font-mono text-[11px] tabular-nums text-accent shadow-[0_0_0_0.5px_var(--color-line)]">
            {timestamp}
          </span>
          <span className="py-0.5 italic text-ink/85">”{quote}”</span>
        </button>
      )}
    </div>
  );
}
