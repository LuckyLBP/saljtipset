import { useCallback, useEffect, useRef, useState } from "react";
import { api, type AppInfo, type CallSummary, errorText, onQueue, onTips, onTranscription, type Phase, type Segment, type Tips } from "./api";
import Setup from "./views/Setup";
import Onboarding from "./views/Onboarding";
import NewCall from "./views/NewCall";
import Sidebar from "./views/Sidebar";
import CallView from "./views/CallView";
import SettingsView from "./views/Settings";
import { Button, ErrorNote, Spinner } from "./ui";

export type View = { name: "new" } | { name: "call"; id: string } | { name: "settings" };

/** What we know about a transcription while it runs, fed by backend events. */
export interface LiveJob {
  callId: string;
  phase: Phase;
  percent: number;
  segments: Segment[];
}

/** Claude requests in flight or failed, per call. */
export type TipsState = Record<string, { running: boolean; error?: string; tips?: Tips }>;

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [view, setView] = useState<View>({ name: "new" });
  const [calls, setCalls] = useState<CallSummary[] | null>(null);
  const [job, setJob] = useState<LiveJob | null>(null);
  // Calls waiting their turn, in order.
  const [queue, setQueue] = useState<string[]>([]);
  const [tipsState, setTipsState] = useState<TipsState>({});
  // Bumped whenever the call list or a call changed in the backend.
  const [revision, setRevision] = useState(0);
  const [jobError, setJobError] = useState<{ callId: string; message: string } | null>(null);
  const [update, setUpdate] = useState<{ version: string; installing?: boolean; error?: string } | null>(null);
  const viewRef = useRef(view);
  viewRef.current = view;

  const refreshInfo = useCallback(async () => {
    try {
      setInfo(await api.appInfo());
    } catch (e) {
      setLoadError(errorText(e));
    }
  }, []);

  useEffect(() => {
    refreshInfo();
    // A transcription may still be running if the window was reloaded.
    api.currentJob().then((id) => id && setJob({ callId: id, phase: "transcribing", percent: 0, segments: [] }));
    api.queueIds().then(setQueue).catch(() => {});
    // Only does anything in builds with the updater turned on; offline is fine.
    const check = () => api.checkUpdate().then((v) => v && setUpdate((u) => u ?? { version: v })).catch(() => {});
    check();
    const timer = setInterval(check, 6 * 60 * 60 * 1000);
    return () => clearInterval(timer);
  }, [refreshInfo]);

  useEffect(() => {
    const un1 = onTranscription((e) => {
      switch (e.kind) {
        case "started":
          setJob({ callId: e.callId, phase: "decoding", percent: 0, segments: [] });
          setRevision((r) => r + 1);
          break;
        case "progress":
          setJob((j) => ({ callId: e.callId, segments: j?.callId === e.callId ? j.segments : [], phase: e.phase, percent: e.percent }));
          break;
        case "segment":
          setJob((j) => {
            const base = j?.callId === e.callId ? j : { callId: e.callId, phase: "transcribing" as Phase, percent: 0, segments: [] };
            return { ...base, segments: [...base.segments, e.segment] };
          });
          break;
        case "done":
          setJob((j) => (j?.callId === e.callId ? null : j));
          setRevision((r) => r + 1);
          break;
        case "error":
          setJob((j) => (j?.callId === e.callId ? null : j));
          setJobError({ callId: e.callId, message: e.message });
          setRevision((r) => r + 1);
          break;
        case "cancelled":
          setJob((j) => (j?.callId === e.callId ? null : j));
          setRevision((r) => r + 1);
          if (viewRef.current.name === "call" && viewRef.current.id === e.callId) setView({ name: "new" });
          break;
      }
    });
    const un2 = onTips((e) => {
      setTipsState((s) => {
        switch (e.kind) {
          case "started":
            return { ...s, [e.callId]: { running: true } };
          case "done":
            return { ...s, [e.callId]: { running: false, tips: e.tips } };
          case "error":
            return { ...s, [e.callId]: { running: false, error: e.message } };
        }
      });
      if (e.kind === "done") setRevision((r) => r + 1);
    });
    const un3 = onQueue(setQueue);
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
    };
  }, []);

  useEffect(() => {
    api.listCalls().then(setCalls).catch(() => setCalls([]));
  }, [revision]);

  const busy = job !== null || queue.length > 0;
  const busyRef = useRef(busy);
  busyRef.current = busy;

  /** Queue files. With nothing running, open the first one so its progress shows. */
  const startFiles = useCallback(async (paths: string[]): Promise<number> => {
    setJobError(null);
    const wasBusy = busyRef.current;
    const ids = await api.startTranscription(paths);
    setRevision((r) => r + 1);
    if (!wasBusy && ids[0]) setView({ name: "call", id: ids[0] });
    return ids.length;
  }, []);

  if (loadError) {
    return (
      <div className="content flex h-full items-center justify-center p-8">
        <ErrorNote>Bubbe kunde inte starta: {loadError}</ErrorNote>
      </div>
    );
  }
  if (!info) {
    return (
      <div className="flex h-full items-center justify-center text-muted" data-tauri-drag-region>
        <Spinner />
      </div>
    );
  }
  if (!info.onboarded) {
    return <Onboarding info={info} onDone={refreshInfo} />;
  }
  if (!info.activeModel) {
    return <Setup info={info} onDone={refreshInfo} />;
  }

  return (
    <div className="flex h-full">
      <Sidebar
        calls={calls}
        view={view}
        job={job}
        queue={queue}
        tipsRunning={Object.entries(tipsState).filter(([, t]) => t.running).map(([id]) => id)}
        onNavigate={setView}
      />

      <main className="content relative flex min-w-0 flex-1 flex-col border-l border-line">
        {update && (
          <div className="flex items-center gap-3 border-b border-line bg-accent-soft px-5 py-2 text-[12.5px]">
            <span className="flex-1">
              {update.error ?? `Bubbe ${update.version} finns. Uppdateringen tar en minut och appen startar om.`}
            </span>
            <Button
              variant="primary"
              size="sm"
              disabled={update.installing || busy}
              onClick={async () => {
                setUpdate({ ...update, installing: true, error: undefined });
                try {
                  await api.installUpdate();
                } catch (e) {
                  setUpdate({ ...update, installing: false, error: errorText(e) });
                }
              }}
            >
              {update.installing ? <Spinner /> : "Uppdatera"}
            </Button>
            <Button variant="plain" size="sm" onClick={() => setUpdate(null)}>
              Senare
            </Button>
          </div>
        )}
        {info.gpuNotice && <div className="border-b border-line bg-accent-soft px-5 py-2 text-[12.5px]">{info.gpuNotice}</div>}

        <div className="min-h-0 flex-1">
          {view.name === "new" && (
            <NewCall
              busy={busy}
              queued={queue.length}
              onFiles={startFiles}
              onOpenJob={() => job && setView({ name: "call", id: job.callId })}
            />
          )}
          {view.name === "call" && (
            <CallView
              key={view.id}
              id={view.id}
              info={info}
              revision={revision}
              job={job?.callId === view.id ? job : null}
              queuePosition={queue.indexOf(view.id) + 1}
              jobError={jobError?.callId === view.id ? jobError.message : null}
              tipsState={tipsState[view.id]}
              onTipsState={(s) => setTipsState((all) => ({ ...all, [view.id]: s }))}
              onDeleted={() => {
                setRevision((r) => r + 1);
                setView({ name: "new" });
              }}
              onChanged={() => setRevision((r) => r + 1)}
              onOpenSettings={() => setView({ name: "settings" })}
            />
          )}
          {view.name === "settings" && (
            <SettingsView
              info={info}
              busy={busy}
              onChanged={refreshInfo}
              onDataDeleted={async () => {
                setTipsState({});
                setRevision((r) => r + 1);
                setView({ name: "new" });
                await refreshInfo();
              }}
            />
          )}
        </div>
      </main>
    </div>
  );
}
