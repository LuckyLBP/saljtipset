import { useEffect, useState } from "react";
import { api, type AppInfo, type DownloadProgress, errorText, formatBytes, type ModelId, onDownload } from "../api";
import { Button, ErrorNote, ProgressBar } from "../ui";
import { Bubbe } from "../mascot";

/** Model cards with download, used by both the first-run setup and Settings. */
export default function ModelPicker({
  info,
  onChanged,
  compact = false,
  busy = false,
}: {
  info: AppInfo;
  onChanged: () => void;
  compact?: boolean;
  busy?: boolean;
}) {
  const [selected, setSelected] = useState<ModelId>(info.activeModel ?? info.hardware.recommended);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [downloading, setDownloading] = useState<ModelId | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const un = onDownload(setProgress);
    return () => {
      un.then((f) => f());
    };
  }, []);

  const download = async (id: ModelId) => {
    setError(null);
    setDownloading(id);
    setProgress(null);
    try {
      await api.downloadModel(id);
      onChanged();
    } catch (e) {
      const msg = errorText(e);
      if (msg !== "Avbrutet") setError(msg);
    } finally {
      setDownloading(null);
      setProgress(null);
    }
  };

  const remove = async (id: ModelId) => {
    setError(null);
    try {
      await api.deleteModel(id);
      onChanged();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const use = async (id: ModelId) => {
    setError(null);
    try {
      await api.setActiveModel(id);
      onChanged();
    } catch (e) {
      setError(errorText(e));
    }
  };

  const gpu = info.hardware.gpu;
  return (
    <div className="space-y-3">
      <div className={`grid gap-3 ${compact ? "grid-cols-1" : "grid-cols-3"}`}>
        {info.models.map((m) => {
          const isSelected = selected === m.id;
          const isActive = info.activeModel === m.id;
          const recommended = info.hardware.recommended === m.id;
          return (
            <div
              key={m.id}
              onClick={() => !downloading && setSelected(m.id)}
              className={`relative rounded-2xl border bg-surface p-4 transition-colors ${
                compact ? "flex flex-wrap items-center gap-x-4 gap-y-3" : ""
              } ${isSelected && !compact ? "border-accent ring-1 ring-accent" : "border-line"}`}
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-[15px] font-semibold">{m.label}</span>
                  {recommended && (
                    <span className="rounded-full bg-accent-soft px-2 py-0.5 text-[11px] font-medium text-accent">
                      Rekommenderas
                    </span>
                  )}
                  {compact && isActive && (
                    <span className="rounded-full bg-sunken px-2 py-0.5 text-[11px] font-medium text-muted">Används</span>
                  )}
                </div>
                <p className="mt-1 text-[12.5px] text-muted">{m.description}</p>
                <p className="mt-2 text-[12px] text-muted">
                  {formatBytes(m.sizeBytes)} · ca {gpu ? m.minutesGpu : m.minutesCpu} min för ett 30-minuterssamtal
                </p>
              </div>
              {compact && (
                <div className="flex shrink-0 items-center gap-2">
                  {m.installed ? (
                    <>
                      {!isActive && (
                        <Button onClick={() => use(m.id)} disabled={busy}>
                          Använd
                        </Button>
                      )}
                      <Button variant="plain" onClick={() => remove(m.id)} disabled={busy || downloading !== null}>
                        Ta bort
                      </Button>
                    </>
                  ) : downloading === m.id ? (
                    <Button onClick={() => api.cancelDownload()}>Avbryt</Button>
                  ) : (
                    <Button onClick={() => download(m.id)} disabled={downloading !== null}>
                      Ladda ner
                    </Button>
                  )}
                </div>
              )}
              {compact && downloading === m.id && (
                <div className="basis-full">
                  <DownloadBar progress={progress} />
                </div>
              )}
            </div>
          );
        })}
      </div>

      {!compact && (
        <div className="space-y-3">
          {downloading ? (
            <div className="space-y-2">
              <DownloadBar progress={progress} />
              <div className="flex justify-end">
                <Button onClick={() => api.cancelDownload()}>Avbryt</Button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between gap-4">
              <p className="text-[12.5px] text-muted">
                {gpu
                  ? `Vi hittade ${info.hardware.gpuName ?? "ett grafikkort"}, så de större modellerna går snabbt.`
                  : "Vi hittade inget grafikkort som kan användas, så transkriberingen körs på processorn."}{" "}
                Du kan byta modell senare under Inställningar.
              </p>
              <Button variant="primary" onClick={() => download(selected)} className="shrink-0 px-4 py-2">
                Ladda ner {info.models.find((m) => m.id === selected)?.label}
              </Button>
            </div>
          )}
        </div>
      )}
      {error && <ErrorNote>{error}</ErrorNote>}
    </div>
  );
}

function DownloadBar({ progress }: { progress: DownloadProgress | null }) {
  const pct = progress && progress.total > 0 ? (progress.downloaded / progress.total) * 100 : 0;
  let label = "Startar nedladdningen…";
  if (progress?.phase === "downloading") {
    label = `${formatBytes(progress.downloaded)} av ${formatBytes(progress.total)}`;
    if (progress.bytesPerSec > 0) label += ` · ${(progress.bytesPerSec / 1e6).toFixed(1).replace(".", ",")} MB/s`;
  } else if (progress?.phase === "verifying") {
    label = "Kontrollerar filen…";
  }
  return (
    <div className="flex items-center gap-3">
      <Bubbe mood={progress?.phase === "verifying" ? "thinking" : "flying"} size={40} />
      <div className="flex-1 space-y-1">
      <ProgressBar value={pct} indeterminate={!progress || progress.phase === "verifying"} />
      <div className="text-[11.5px] text-muted">{label}</div>
      </div>
    </div>
  );
}
