import { useCallback, useEffect, useRef, useState } from "react";
import { api, type AppInfo, type CallSummary, errorText, onTips, onTranscription, type Phase, type Segment, type Tips } from "./api";
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
    // Only does anything in builds with the updater turned on; offline is fine.
    api.checkUpdate().then((v) => v && setUpdate({ version: v })).catch(() => {});
  }, [refreshInfo]);

  useEffect(() => {
    const un1 = onTranscription((e) => {
      switch (e.kind) {
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
          setJob(null);
          setRevision((r) => r + 1);
          break;
        case "error":
          setJob(null);
          setJobError({ callId: e.callId, message: e.message });
          setRevision((r) => r + 1);
          break;
        case "cancelled":
          setJob(null);
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
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
    };
  }, []);

  useEffect(() => {
    api.listCalls().then(setCalls).catch(() => setCalls([]));
  }, [revision]);

  const startFile = useCallback(async (path: string) => {
    setJobError(null);
    const id = await api.startTranscription(path);
    setJob({ callId: id, phase: "decoding", percent: 0, segments: [] });
    setRevision((r) => r + 1);
    setView({ name: "call", id });
  }, []);

  if (loadError) {
    return (
      <div className="content flex h-full items-center justify-center p-8">
        <ErrorNote>Säljtipset kunde inte starta: {loadError}</ErrorNote>
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
        tipsRunning={Object.entries(tipsState).filter(([, t]) => t.running).map(([id]) => id)}
        onNavigate={setView}
      />

      <main className="content relative flex min-w-0 flex-1 flex-col border-l border-line">
        {update && (
          <div className="flex items-center gap-3 border-b border-line bg-accent-soft px-5 py-2 text-[12.5px]">
            <span className="flex-1">
              {update.error ?? `Säljtipset ${update.version} finns. Uppdateringen tar en minut och appen startar om.`}
            </span>
            <Button
              variant="primary"
              size="sm"
              disabled={update.installing || job !== null}
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
          {view.name === "new" && <NewCall busy={job !== null} onFile={startFile} onOpenJob={() => job && setView({ name: "call", id: job.callId })} />}
          {view.name === "call" && (
            <CallView
              key={view.id}
              id={view.id}
              info={info}
              revision={revision}
              job={job?.callId === view.id ? job : null}
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
              busy={job !== null}
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
