pub mod ai;
pub mod audio;
pub mod claude;
pub mod db;
pub mod export;
pub mod gpu;
pub mod models;
pub mod people;
pub mod sales;
pub mod secrets;
pub mod tips;
pub mod transcribe;
pub mod transcript;
pub mod vad;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

use db::{Call, CallSummary, Db};
use models::ModelId;
use transcribe::{ContextCache, Phase, Sink};
use transcript::Segment;

const SETTING_MODEL: &str = "active_model";
const SETTING_CLAUDE_MODEL: &str = "claude_model";
const SETTING_AUTO_TIPS: &str = "auto_tips";
const SETTING_USE_GPU: &str = "use_gpu";
const SETTING_AI_PROVIDER: &str = "ai_provider";
const SETTING_THEME: &str = "theme";
const SETTING_PROFILE: &str = "profile";

struct Job {
    call_id: String,
    cancel: Arc<AtomicBool>,
}

pub struct AppState {
    app_data: PathBuf,
    db: Mutex<Db>,
    contexts: Arc<ContextCache>,
    job: Mutex<Option<Job>>,
    download: Mutex<Option<Arc<AtomicBool>>>,
    gpu_notice: Mutex<Option<String>>,
    /// Claude Code / Codex found on this computer; looked up once, refreshed on request.
    detected: Mutex<Option<ai::Detected>>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl AppState {
    fn setting(&self, key: &str) -> Option<String> {
        self.db.lock().unwrap().get_setting(key)
    }
    fn use_gpu(&self) -> bool {
        gpu::compiled_with_gpu() && self.setting(SETTING_USE_GPU).as_deref() != Some("false")
    }
    fn active_model(&self) -> Option<ModelId> {
        self.setting(SETTING_MODEL).and_then(|s| ModelId::parse(&s)).filter(|id| models::is_installed(&self.app_data, *id))
    }
    fn claude_model(&self) -> String {
        self.setting(SETTING_CLAUDE_MODEL).filter(|s| !s.trim().is_empty()).unwrap_or_else(|| claude::DEFAULT_MODEL.into())
    }
    /// "system", "light" or "dark".
    fn theme(&self) -> String {
        self.setting(SETTING_THEME).unwrap_or_else(|| "system".into())
    }
    /// The seller's profile from onboarding; `None` until onboarding is done.
    fn profile(&self) -> Option<people::Profile> {
        self.setting(SETTING_PROFILE).and_then(|s| serde_json::from_str(&s).ok())
    }
    /// "auto" or a [`ai::Provider`] id.
    fn ai_provider_setting(&self) -> String {
        self.setting(SETTING_AI_PROVIDER).unwrap_or_else(|| "auto".into())
    }
    fn auto_tips(&self) -> bool {
        self.setting(SETTING_AUTO_TIPS).as_deref() == Some("true")
    }
}

// ---------- app state for the UI ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelInfo {
    id: ModelId,
    label: &'static str,
    size_bytes: u64,
    minutes_gpu: &'static str,
    minutes_cpu: &'static str,
    description: &'static str,
    installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    theme: String,
    claude_model: String,
    auto_tips: bool,
    use_gpu: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    models: Vec<ModelInfo>,
    active_model: Option<ModelId>,
    hardware: models::Hardware,
    has_api_key: bool,
    /// "auto", "api", "claude-code" or "codex".
    ai_provider: String,
    /// What will actually answer right now, if anything.
    active_provider: Option<ai::Provider>,
    detected: ai::Detected,
    settings: Settings,
    gpu_compiled: bool,
    gpu_notice: Option<String>,
    ffmpeg_available: bool,
    version: String,
    data_dir: String,
    /// False until the user has been through onboarding.
    onboarded: bool,
    profile: people::Profile,
}

async fn detected(state: &AppState) -> ai::Detected {
    if let Some(d) = state.detected.lock().unwrap().clone() {
        return d;
    }
    let d = ai::detect().await;
    *state.detected.lock().unwrap() = Some(d.clone());
    d
}

/// The backend to use for tips and chat, following the user's choice ("auto" picks the API
/// key first, then a Claude subscription, then ChatGPT).
async fn resolve_backend(state: &AppState) -> CmdResult<ai::Backend> {
    let found = detected(state).await;
    let choice = state.ai_provider_setting();
    let key = secrets::get_api_key().ok().flatten();
    let claude = found.claude_code.filter(|c| c.logged_in);
    let codex = found.codex.filter(|c| c.logged_in);
    let api = key.map(|key| ai::Backend::Api { key, model: state.claude_model() });
    let claude = claude.map(|c| ai::Backend::ClaudeCode { path: c.path, model: state.claude_model() });
    let codex = codex.map(|c| ai::Backend::Codex { path: c.path });
    match ai::Provider::parse(&choice) {
        Some(ai::Provider::Api) => api.ok_or_else(|| "Lägg till din Anthropic API-nyckel under Inställningar först.".into()),
        Some(ai::Provider::ClaudeCode) => {
            claude.ok_or_else(|| "Claude Code hittades inte eller är inte inloggat. Kör `claude` i Terminalen och logga in.".into())
        }
        Some(ai::Provider::Codex) => {
            codex.ok_or_else(|| "Codex hittades inte eller är inte inloggat. Kör `codex login` i Terminalen.".into())
        }
        None => api.or(claude).or(codex).ok_or_else(|| {
            "Ingen AI är kopplad. Lägg till en API-nyckel, eller logga in i Claude Code eller Codex på datorn, under Inställningar."
                .into()
        }),
    }
}

#[tauri::command]
async fn get_app_info(app: AppHandle, state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let models = models::CATALOG
        .iter()
        .map(|m| ModelInfo {
            id: m.id,
            label: m.label,
            size_bytes: m.approx_bytes,
            minutes_gpu: m.minutes_gpu,
            minutes_cpu: m.minutes_cpu,
            description: m.description,
            installed: models::is_installed(&state.app_data, m.id),
        })
        .collect();
    Ok(AppInfo {
        models,
        active_model: state.active_model(),
        hardware: models::detect_hardware(state.use_gpu()),
        has_api_key: secrets::get_api_key().ok().flatten().is_some(),
        ai_provider: state.ai_provider_setting(),
        active_provider: resolve_backend(&state).await.ok().map(|b| b.provider()),
        detected: detected(&state).await,
        settings: Settings {
            theme: state.theme(),
            claude_model: state.claude_model(),
            auto_tips: state.auto_tips(),
            use_gpu: state.use_gpu(),
        },
        onboarded: state.profile().is_some(),
        profile: state.profile().unwrap_or_default(),
        gpu_compiled: gpu::compiled_with_gpu(),
        gpu_notice: state.gpu_notice.lock().unwrap().clone(),
        ffmpeg_available: audio::find_ffmpeg().is_some(),
        version: app.package_info().version.to_string(),
        data_dir: state.app_data.display().to_string(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsUpdate {
    claude_model: Option<String>,
    auto_tips: Option<bool>,
    use_gpu: Option<bool>,
    ai_provider: Option<String>,
}

#[tauri::command]
fn update_settings(state: State<'_, AppState>, update: SettingsUpdate) -> CmdResult<()> {
    let db = state.db.lock().unwrap();
    if let Some(m) = update.claude_model {
        db.set_setting(SETTING_CLAUDE_MODEL, m.trim()).map_err(err)?;
    }
    if let Some(a) = update.auto_tips {
        db.set_setting(SETTING_AUTO_TIPS, if a { "true" } else { "false" }).map_err(err)?;
    }
    if let Some(p) = update.ai_provider {
        if p != "auto" && ai::Provider::parse(&p).is_none() {
            return Err(format!("Okänd AI-tjänst: {p}"));
        }
        db.set_setting(SETTING_AI_PROVIDER, &p).map_err(err)?;
    }
    if let Some(g) = update.use_gpu {
        db.set_setting(SETTING_USE_GPU, if g { "true" } else { "false" }).map_err(err)?;
        drop(db);
        // The cached model was loaded for the old setting.
        state.contexts.clear();
        if g {
            *state.gpu_notice.lock().unwrap() = None;
        }
    }
    Ok(())
}

// ---------- models ----------

#[tauri::command]
async fn download_model(app: AppHandle, state: State<'_, AppState>, id: ModelId) -> CmdResult<()> {
    let cancel = {
        let mut slot = state.download.lock().unwrap();
        if slot.is_some() {
            return Err("En nedladdning pågår redan.".into());
        }
        let c = Arc::new(AtomicBool::new(false));
        *slot = Some(c.clone());
        c
    };
    let result = models::download(&state.app_data, id, &cancel, |p| {
        let _ = app.emit("model-download", &p);
    })
    .await;
    *state.download.lock().unwrap() = None;
    result?;
    // The first model downloaded becomes the active one; a new pick replaces it.
    state.db.lock().unwrap().set_setting(SETTING_MODEL, id.as_str()).map_err(err)?;
    Ok(())
}

#[tauri::command]
fn cancel_download(state: State<'_, AppState>) {
    if let Some(c) = state.download.lock().unwrap().as_ref() {
        c.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
fn set_active_model(state: State<'_, AppState>, id: ModelId) -> CmdResult<()> {
    if !models::is_installed(&state.app_data, id) {
        return Err("Modellen är inte nedladdad.".into());
    }
    state.db.lock().unwrap().set_setting(SETTING_MODEL, id.as_str()).map_err(err)
}

#[tauri::command]
fn delete_model(state: State<'_, AppState>, id: ModelId) -> CmdResult<()> {
    if state.job.lock().unwrap().is_some() {
        return Err("Vänta tills transkriberingen är klar.".into());
    }
    state.contexts.clear();
    models::delete(&state.app_data, id).map_err(|e| format!("Kunde inte ta bort modellen: {e}"))
}

// ---------- transcription ----------

struct EventSink {
    app: AppHandle,
    call_id: String,
    last: Mutex<(Instant, i32)>,
}

impl Sink for EventSink {
    fn progress(&self, phase: Phase, percent: f32) {
        let mut last = self.last.lock().unwrap();
        let whole = percent.floor() as i32;
        // Throttle: at most ~10 events/s, and only when the integer percent moves.
        if whole == last.1 && last.0.elapsed() < Duration::from_millis(500) {
            return;
        }
        if last.0.elapsed() < Duration::from_millis(100) && whole < 100 {
            return;
        }
        *last = (Instant::now(), whole);
        let _ = self.app.emit(
            "transcription",
            json!({"callId": self.call_id, "kind": "progress", "phase": phase, "percent": percent}),
        );
    }
    fn segment(&self, segment: &Segment) {
        let _ = self.app.emit("transcription", json!({"callId": self.call_id, "kind": "segment", "segment": segment}));
    }
}

fn file_title(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("Samtal").replace(['_', '-'], " ")
}

#[tauri::command]
fn start_transcription(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<String> {
    let audio_path = PathBuf::from(&path);
    if !audio_path.is_file() {
        return Err("Filen hittades inte.".into());
    }
    let model_id = state.active_model().ok_or("Ladda ner en talmodell först.")?;
    let model_path = models::model_path(&state.app_data, model_id);

    let mut job = state.job.lock().unwrap();
    if job.is_some() {
        return Err("En transkribering pågår redan. Vänta tills den är klar eller avbryt den.".into());
    }
    let call = Call {
        id: uuid::Uuid::new_v4().to_string(),
        title: file_title(&audio_path),
        file_path: path.clone(),
        file_name: audio_path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        duration_ms: 0,
        channels: 1,
        model: model_id.as_str().into(),
        status: "transcribing".into(),
        error: None,
        segments: vec![],
        tips: None,
        roles: vec![],
        labels: vec![],
        stats: None,
    };
    state.db.lock().unwrap().insert_call(&call).map_err(err)?;
    let cancel = Arc::new(AtomicBool::new(false));
    *job = Some(Job { call_id: call.id.clone(), cancel: cancel.clone() });
    drop(job);

    let opts = transcribe::Options { use_gpu: state.use_gpu(), ..Default::default() };
    let call_id = call.id.clone();
    let contexts = state.contexts.clone();
    std::thread::Builder::new()
        .name("transcribe".into())
        .spawn(move || {
            let sink = Arc::new(EventSink { app: app.clone(), call_id: call_id.clone(), last: Mutex::new((Instant::now(), -1)) });
            let result = transcribe::transcribe_file(&contexts, &model_path, &audio_path, &opts, cancel, sink);
            let state = app.state::<AppState>();
            *state.job.lock().unwrap() = None;
            let db = state.db.lock().unwrap();
            match result {
                Ok(out) => {
                    let _ = db.finish_transcription(&call_id, &out.segments, out.duration_ms, out.channels);
                    drop(db);
                    let _ = app.emit(
                        "transcription",
                        json!({"callId": call_id, "kind": "done", "segments": out.segments, "durationMs": out.duration_ms, "channels": out.channels}),
                    );
                    if state.auto_tips() {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            // Quietly skip when no AI is connected; the tips panel explains why.
                            if resolve_backend(&app.state::<AppState>()).await.is_ok() {
                                let _ = run_tips(&app, &call_id).await;
                            }
                        });
                    }
                }
                Err(transcribe::TranscribeError::Cancelled)
                | Err(transcribe::TranscribeError::Audio(audio::AudioError::Cancelled)) => {
                    let _ = db.delete(&call_id);
                    let _ = app.emit("transcription", json!({"callId": call_id, "kind": "cancelled"}));
                }
                Err(e) => {
                    let msg = e.to_string();
                    let _ = db.set_status(&call_id, "error", Some(&msg));
                    let _ = app.emit("transcription", json!({"callId": call_id, "kind": "error", "message": msg}));
                }
            }
        })
        .map_err(err)?;
    Ok(call.id)
}

#[tauri::command]
fn cancel_transcription(state: State<'_, AppState>) {
    if let Some(job) = state.job.lock().unwrap().as_ref() {
        job.cancel.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
fn current_job(state: State<'_, AppState>) -> Option<String> {
    state.job.lock().unwrap().as_ref().map(|j| j.call_id.clone())
}

// ---------- calls ----------

#[tauri::command]
fn list_calls(state: State<'_, AppState>) -> CmdResult<Vec<CallSummary>> {
    state.db.lock().unwrap().list().map_err(err)
}

#[tauri::command]
fn get_call(state: State<'_, AppState>, id: String) -> CmdResult<Call> {
    let mut call = state.db.lock().unwrap().get(&id).map_err(err)?.ok_or("Samtalet finns inte.")?;
    let profile = state.profile().unwrap_or_default();
    call.roles = people::roles(&call.segments, call.tips.as_ref());
    call.labels = people::labels(&call.segments, &call.roles, &profile, call.tips.as_ref());
    call.stats = people::stats(&call.segments, &call.roles);
    Ok(call)
}

/// Apply "system", "light" or "dark" to the window. The web view's prefers-color-scheme
/// follows the window, so the CSS needs nothing else.
fn apply_theme(app: &AppHandle, theme: &str) {
    let t = match theme {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    };
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_theme(t);
    }
}

#[tauri::command]
fn set_theme(app: AppHandle, state: State<'_, AppState>, theme: String) -> CmdResult<()> {
    if !["system", "light", "dark"].contains(&theme.as_str()) {
        return Err(format!("Okänt utseende: {theme}"));
    }
    state.db.lock().unwrap().set_setting(SETTING_THEME, &theme).map_err(err)?;
    apply_theme(&app, &theme);
    Ok(())
}

#[tauri::command]
fn save_profile(state: State<'_, AppState>, profile: people::Profile) -> CmdResult<()> {
    let profile = people::Profile {
        name: profile.name.trim().to_string(),
        gender: profile.gender.trim().to_string(),
        company: profile.company.trim().to_string(),
        offer: profile.offer.trim().to_string(),
    };
    if profile.name.is_empty() {
        return Err("Skriv ditt namn först.".into());
    }
    state.db.lock().unwrap().set_setting(SETTING_PROFILE, &serde_json::to_string(&profile).unwrap()).map_err(err)
}

#[tauri::command]
fn rename_call(state: State<'_, AppState>, id: String, title: String) -> CmdResult<()> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Namnet kan inte vara tomt.".into());
    }
    state.db.lock().unwrap().rename(&id, title).map_err(err)
}

#[tauri::command]
fn delete_call(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    if let Some(job) = state.job.lock().unwrap().as_ref() {
        if job.call_id == id {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
    state.db.lock().unwrap().delete(&id).map_err(err)
}

#[tauri::command]
fn transcript_text(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let call = get_call(state, id)?;
    Ok(transcript::format_for_copy(&call.segments, &call.labels))
}

#[tauri::command]
fn tips_text(state: State<'_, AppState>, id: String) -> CmdResult<String> {
    let call = get_call(state, id)?;
    let tips = call.tips.as_ref().ok_or("Inga säljtips än.")?;
    Ok(tips::format_tips_plain(tips, &call.title))
}

#[tauri::command]
fn export_call(state: State<'_, AppState>, id: String, format: export::Format, path: String) -> CmdResult<()> {
    let call = get_call(state, id)?;
    export::export(&call, format, Path::new(&path))
}

// ---------- AI: tips and chat ----------

#[tauri::command]
async fn refresh_ai(state: State<'_, AppState>) -> CmdResult<ai::Detected> {
    *state.detected.lock().unwrap() = None;
    Ok(detected(&state).await)
}

#[tauri::command]
fn chat_history(state: State<'_, AppState>, id: String) -> CmdResult<Vec<db::ChatMessage>> {
    state.db.lock().unwrap().chat_history(&id).map_err(err)
}

#[tauri::command]
async fn send_chat(state: State<'_, AppState>, id: String, question: String) -> CmdResult<Vec<db::ChatMessage>> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err("Skriv en fråga först.".into());
    }
    let (call, history) = {
        let db = state.db.lock().unwrap();
        let call = db.get(&id).map_err(err)?.ok_or("Samtalet finns inte.")?;
        (call, db.chat_history(&id).map_err(err)?)
    };
    if call.segments.is_empty() {
        return Err("Samtalet har ingen text att prata om än.".into());
    }
    let backend = resolve_backend(&state).await?;
    let turns: Vec<ai::Turn> = history
        .iter()
        .map(|m| ai::Turn { role: if m.role == "user" { ai::Role::User } else { ai::Role::Assistant }, text: m.text.clone() })
        .collect();
    let context = ai::chat_context(&call.title, &call.segments, call.tips.as_ref(), &state.profile().unwrap_or_default());
    let answer = ai::complete(&backend, &ai::chat_prompt(context, &turns, &question)).await?;
    state.db.lock().unwrap().add_chat_exchange(&id, &question, answer.trim()).map_err(err)
}

#[tauri::command]
fn clear_chat(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db.lock().unwrap().clear_chat(&id).map_err(err)
}

async fn run_tips(app: &AppHandle, call_id: &str) -> CmdResult<tips::Tips> {
    let state = app.state::<AppState>();
    let call = state.db.lock().unwrap().get(call_id).map_err(err)?.ok_or("Samtalet finns inte.")?;
    let backend = match resolve_backend(&state).await {
        Ok(b) => b,
        Err(e) => {
            let _ = app.emit("tips", json!({"callId": call_id, "kind": "error", "message": e}));
            return Err(e);
        }
    };
    let _ = app.emit("tips", json!({"callId": call_id, "kind": "started", "provider": backend.provider()}));
    let profile = state.profile().unwrap_or_default();
    match ai::generate_tips(&backend, &call.segments, call.channels > 1, &profile).await {
        Ok(tips) => {
            state.db.lock().unwrap().save_tips(call_id, &tips).map_err(err)?;
            let _ = app.emit("tips", json!({"callId": call_id, "kind": "done", "tips": tips}));
            Ok(tips)
        }
        Err(e) => {
            let _ = app.emit("tips", json!({"callId": call_id, "kind": "error", "message": e}));
            Err(e)
        }
    }
}

#[tauri::command]
async fn generate_tips(app: AppHandle, id: String) -> CmdResult<tips::Tips> {
    run_tips(&app, &id).await
}

#[tauri::command]
fn save_api_key(key: String) -> CmdResult<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err("Klistra in en nyckel först.".into());
    }
    secrets::set_api_key(key)
}

#[tauri::command]
fn remove_api_key() -> CmdResult<()> {
    secrets::delete_api_key()
}

/// Tests `key` if given, otherwise the stored key.
#[tauri::command]
async fn test_api_key(state: State<'_, AppState>, key: Option<String>) -> CmdResult<()> {
    let key = match key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        Some(k) => k,
        None => secrets::get_api_key()?.ok_or("Ingen nyckel sparad.")?,
    };
    let model = state.claude_model();
    claude::test_key(&key, &model).await
}

// ---------- data ----------

#[tauri::command]
fn delete_all_data(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(job) = state.job.lock().unwrap().as_ref() {
        job.cancel.store(true, Ordering::Relaxed);
    }
    if let Some(c) = state.download.lock().unwrap().as_ref() {
        c.store(true, Ordering::Relaxed);
    }
    state.contexts.clear();
    state.db.lock().unwrap().delete_all().map_err(err)?;
    let dir = models::models_dir(&state.app_data);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("Kunde inte ta bort modellerna: {e}"))?;
    }
    secrets::delete_api_key()
}

// ---------- updates (only with the `updater` feature) ----------

#[cfg(feature = "updater")]
#[tauri::command]
async fn check_update(app: AppHandle) -> CmdResult<Option<String>> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(err)?.check().await.map_err(err)?;
    Ok(update.map(|u| u.version))
}

#[cfg(feature = "updater")]
#[tauri::command]
async fn install_update(app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_updater::UpdaterExt;
    let Some(update) = app.updater().map_err(err)?.check().await.map_err(err)? else {
        return Ok(());
    };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| format!("Uppdateringen misslyckades: {e}"))?;
    app.restart();
}

/// Without the updater there is never an update; the UI calls this unconditionally.
#[cfg(not(feature = "updater"))]
#[tauri::command]
async fn check_update() -> CmdResult<Option<String>> {
    Ok(None)
}

#[cfg(not(feature = "updater"))]
#[tauri::command]
async fn install_update() -> CmdResult<()> {
    Err("Uppdateringar är inte aktiverade i den här versionen.".into())
}

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init());
    #[cfg(feature = "updater")]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;
            gpu::init(&app_data);
            let db = Db::open(&app_data.join("saljtipset.sqlite"))?;

            let mut gpu_notice = None;
            if gpu::previous_run_crashed_in_gpu_init() {
                db.set_setting(SETTING_USE_GPU, "false")?;
                gpu_notice = Some(
                    "Säljtipset stängdes oväntat när grafikkortet startades förra gången. Transkriberingen använder nu processorn. Du kan slå på grafikkortet igen under Inställningar."
                        .to_string(),
                );
            }
            if db.get_setting(SETTING_USE_GPU).as_deref() == Some("false") {
                gpu::disable_for_process();
            }

            let theme = db.get_setting(SETTING_THEME).unwrap_or_else(|| "system".into());
            app.manage(AppState {
                app_data,
                db: Mutex::new(db),
                contexts: Arc::new(ContextCache::default()),
                job: Mutex::new(None),
                download: Mutex::new(None),
                gpu_notice: Mutex::new(gpu_notice),
                detected: Mutex::new(None),
            });
            apply_theme(app.handle(), &theme);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            set_theme,
            save_profile,
            refresh_ai,
            chat_history,
            send_chat,
            clear_chat,
            update_settings,
            download_model,
            cancel_download,
            set_active_model,
            delete_model,
            start_transcription,
            cancel_transcription,
            current_job,
            list_calls,
            get_call,
            rename_call,
            delete_call,
            transcript_text,
            tips_text,
            export_call,
            generate_tips,
            save_api_key,
            remove_api_key,
            test_api_key,
            delete_all_data,
            check_update,
            install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
