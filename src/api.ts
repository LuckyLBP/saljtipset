import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ModelId = "bast" | "snabb" | "latt";

export interface ModelInfo {
  id: ModelId;
  label: string;
  sizeBytes: number;
  minutesGpu: string;
  minutesCpu: string;
  description: string;
  installed: boolean;
}

export interface Hardware {
  gpu: boolean;
  gpuName: string | null;
  totalMemoryGb: number;
  cpuCores: number;
  recommended: ModelId;
}

export type Provider = "api" | "claude-code" | "codex";
export type ProviderChoice = "auto" | Provider;

export interface CliInfo {
  path: string;
  loggedIn: boolean;
  detail: string | null;
}

export interface Detected {
  claudeCode: CliInfo | null;
  codex: CliInfo | null;
}

export const PROVIDER_LABEL: Record<Provider, string> = {
  api: "Anthropic API-nyckel",
  "claude-code": "Claude-prenumeration",
  codex: "ChatGPT-prenumeration",
};

/** In a sentence: "Använder din Claude-prenumeration." */
export const PROVIDER_PHRASE: Record<Provider, string> = {
  api: "din API-nyckel",
  "claude-code": "din Claude-prenumeration",
  codex: "din ChatGPT-prenumeration",
};

/** Who receives the transcript text with each provider. */
export const PROVIDER_VENDOR: Record<Provider, string> = { api: "Anthropic", "claude-code": "Anthropic", codex: "OpenAI" };

export interface ChatMessage {
  id: number;
  role: "user" | "assistant";
  text: string;
  createdAt: string;
}

export type Theme = "system" | "light" | "dark";
export type Gender = "kvinna" | "man" | "annat" | "";

export interface Profile {
  name: string;
  gender: Gender;
  company: string;
  offer: string;
}

export interface CallStats {
  sellerTalkShare: number;
  sellerQuestions: number;
  customerQuestions: number;
  longestMonologueMs: number;
  exact: boolean;
}

export interface Settings {
  theme: Theme;
  claudeModel: string;
  autoTips: boolean;
  useGpu: boolean;
}

export interface AppInfo {
  models: ModelInfo[];
  activeModel: ModelId | null;
  hardware: Hardware;
  hasApiKey: boolean;
  aiProvider: ProviderChoice;
  activeProvider: Provider | null;
  detected: Detected;
  settings: Settings;
  gpuCompiled: boolean;
  gpuNotice: string | null;
  ffmpegAvailable: boolean;
  version: string;
  dataDir: string;
  onboarded: boolean;
  updaterEnabled: boolean;
  profile: Profile;
}

export interface Segment {
  id: number;
  startMs: number;
  endMs: number;
  text: string;
  speaker: string | null;
}

export interface Evidence {
  quote: string;
  timestamp: string;
  segmentId: number | null;
}

export interface Tips {
  speakers: { segmentId: number; role: string }[];
  channelRoles: { speaker: string; role: string }[];
  summary: string;
  sellingPoints: (Evidence & { title: string; explanation: string })[];
  customerNeeds: (Evidence & { need: string })[];
  objections: (Evidence & { objection: string; suggestedAnswer: string })[];
  missedOpportunities: { description: string; suggestion: string }[];
  nextSteps: string[];
  customerName: string;
  customerCompany: string;
  droppedPoints: number;
}

export type CallStatus = "transcribing" | "done" | "error" | "cancelled";

export interface CallSummary {
  id: string;
  title: string;
  fileName: string;
  createdAt: string;
  durationMs: number;
  status: CallStatus;
  hasTips: boolean;
}

export interface Call {
  id: string;
  title: string;
  filePath: string;
  fileName: string;
  createdAt: string;
  durationMs: number;
  channels: number;
  model: string;
  status: CallStatus;
  error: string | null;
  segments: Segment[];
  tips: Tips | null;
  /** "Säljare" / "Kund" per segment id, once known. */
  roles: (string | null)[];
  /** The name to show per segment id (your name, the customer's, a role or "Talare N"). */
  labels: (string | null)[];
  stats: CallStats | null;
}

export type Phase = "decoding" | "loading" | "transcribing";

export type TranscriptionEvent =
  | { callId: string; kind: "progress"; phase: Phase; percent: number }
  | { callId: string; kind: "segment"; segment: Segment }
  | { callId: string; kind: "done"; segments: Segment[]; durationMs: number; channels: number }
  | { callId: string; kind: "error"; message: string }
  | { callId: string; kind: "cancelled" };

export type TipsEvent =
  | { callId: string; kind: "started" }
  | { callId: string; kind: "done"; tips: Tips }
  | { callId: string; kind: "error"; message: string };

export interface DownloadProgress {
  model: ModelId;
  downloaded: number;
  total: number;
  bytesPerSec: number;
  phase: "downloading" | "verifying" | "done";
}

export type ExportFormat = "txt" | "md" | "docx";

export const api = {
  appInfo: () => invoke<AppInfo>("get_app_info"),
  updateSettings: (update: Partial<Settings> & { aiProvider?: ProviderChoice }) => invoke<void>("update_settings", { update }),
  downloadModel: (id: ModelId) => invoke<void>("download_model", { id }),
  cancelDownload: () => invoke<void>("cancel_download"),
  setActiveModel: (id: ModelId) => invoke<void>("set_active_model", { id }),
  deleteModel: (id: ModelId) => invoke<void>("delete_model", { id }),
  startTranscription: (path: string) => invoke<string>("start_transcription", { path }),
  cancelTranscription: () => invoke<void>("cancel_transcription"),
  currentJob: () => invoke<string | null>("current_job"),
  listCalls: () => invoke<CallSummary[]>("list_calls"),
  getCall: (id: string) => invoke<Call>("get_call", { id }),
  renameCall: (id: string, title: string) => invoke<void>("rename_call", { id, title }),
  deleteCall: (id: string) => invoke<void>("delete_call", { id }),
  transcriptText: (id: string) => invoke<string>("transcript_text", { id }),
  tipsText: (id: string) => invoke<string>("tips_text", { id }),
  exportCall: (id: string, format: ExportFormat, path: string) => invoke<void>("export_call", { id, format, path }),
  generateTips: (id: string) => invoke<Tips>("generate_tips", { id }),
  saveApiKey: (key: string) => invoke<void>("save_api_key", { key }),
  removeApiKey: () => invoke<void>("remove_api_key"),
  testApiKey: (key?: string) => invoke<void>("test_api_key", { key: key ?? null }),
  deleteAllData: () => invoke<void>("delete_all_data"),
  setTheme: (theme: Theme) => invoke<void>("set_theme", { theme }),
  saveProfile: (profile: Profile) => invoke<void>("save_profile", { profile }),
  refreshAi: () => invoke<Detected>("refresh_ai"),
  chatHistory: (id: string) => invoke<ChatMessage[]>("chat_history", { id }),
  sendChat: (id: string, question: string) => invoke<ChatMessage[]>("send_chat", { id, question }),
  clearChat: (id: string) => invoke<void>("clear_chat", { id }),
  checkUpdate: () => invoke<string | null>("check_update"),
  installUpdate: () => invoke<void>("install_update"),
};

export const onTranscription = (cb: (e: TranscriptionEvent) => void): Promise<UnlistenFn> =>
  listen<TranscriptionEvent>("transcription", (e) => cb(e.payload));
export const onTips = (cb: (e: TipsEvent) => void): Promise<UnlistenFn> => listen<TipsEvent>("tips", (e) => cb(e.payload));
export const onDownload = (cb: (e: DownloadProgress) => void): Promise<UnlistenFn> =>
  listen<DownloadProgress>("model-download", (e) => cb(e.payload));

/** Error text from a rejected `invoke` (our commands reject with a Swedish string). */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Något gick fel.";
}

export function formatTimestamp(ms: number): string {
  const s = Math.floor(ms / 1000);
  return `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

export function formatDuration(ms: number): string {
  const totalMin = Math.round(ms / 60000);
  if (ms < 60000) return `${Math.max(1, Math.round(ms / 1000))} s`;
  if (totalMin < 60) return `${totalMin} min`;
  return `${Math.floor(totalMin / 60)} h ${totalMin % 60} min`;
}

export function formatBytes(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toLocaleString("sv-SE", { maximumFractionDigits: 1 })} GB`;
  return `${Math.round(n / 1e6).toLocaleString("sv-SE")} MB`;
}

export function formatDate(iso: string): string {
  return new Date(iso).toLocaleString("sv-SE", { dateStyle: "medium", timeStyle: "short" });
}

export const AUDIO_EXTENSIONS = ["mp3", "m4a", "wav", "ogg", "oga", "webm", "mp4", "aac", "flac", "opus"];
