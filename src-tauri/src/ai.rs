//! Which AI answers: an Anthropic API key, or a subscription the user already has on this
//! computer through the Claude Code CLI (Claude Pro/Max) or the Codex CLI (ChatGPT).
//!
//! The CLIs run headless in an empty temporary folder with every tool, MCP server, setting
//! and slash command turned off, so they only see the text we pipe in. Only transcript text
//! (and earlier tips/chat) is sent, exactly as with the API key.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::people::Profile;
use crate::sales;
use crate::tips::{self, Tips};
use crate::transcript::{Segment, format_for_model};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Turn {
    pub role: Role,
    pub text: String,
}

/// One request, independent of who answers it.
#[derive(Debug, Clone)]
pub struct Prompt {
    pub system: String,
    /// Large, stable text (the transcript) placed before the first turn.
    pub context: Option<String>,
    /// Alternating turns starting and ending with the user.
    pub turns: Vec<Turn>,
    pub schema: Option<Value>,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Api,
    ClaudeCode,
    Codex,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Api => "api",
            Provider::ClaudeCode => "claude-code",
            Provider::Codex => "codex",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        [Provider::Api, Provider::ClaudeCode, Provider::Codex].into_iter().find(|p| p.as_str() == s)
    }
    /// Who receives the text, for messages to the user.
    pub fn vendor(self) -> &'static str {
        match self {
            Provider::Api | Provider::ClaudeCode => "Anthropic",
            Provider::Codex => "OpenAI",
        }
    }
}

/// A CLI found on this computer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInfo {
    pub path: PathBuf,
    pub logged_in: bool,
    /// e.g. "Claude Max" or "Logged in using ChatGPT".
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub claude_code: Option<CliInfo>,
    pub codex: Option<CliInfo>,
}

/// A resolved way to get an answer.
#[derive(Debug, Clone)]
pub enum Backend {
    Api { key: String, model: String },
    ClaudeCode { path: PathBuf, model: String },
    Codex { path: PathBuf },
}

impl Backend {
    pub fn provider(&self) -> Provider {
        match self {
            Backend::Api { .. } => Provider::Api,
            Backend::ClaudeCode { .. } => Provider::ClaudeCode,
            Backend::Codex { .. } => Provider::Codex,
        }
    }
}

// ---------- detection ----------

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// Places the CLIs get installed. GUI apps on macOS don't inherit the shell's PATH, so the
/// usual install folders are listed explicitly (native installer, Homebrew, npm, nvm, bun, volta).
fn candidates(name: &str) -> Vec<PathBuf> {
    let exe_names: Vec<String> = if cfg!(windows) {
        vec![format!("{name}.exe"), format!("{name}.cmd")]
    } else {
        vec![name.to_string()]
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(h) = home() {
        dirs.push(h.join(".local/bin"));
        dirs.push(h.join(".claude/local"));
        dirs.push(h.join(".npm-global/bin"));
        dirs.push(h.join(".bun/bin"));
        dirs.push(h.join(".volta/bin"));
        dirs.push(h.join(".codex/bin"));
        if let Ok(entries) = std::fs::read_dir(h.join(".nvm/versions/node")) {
            let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path().join("bin")).collect();
            versions.sort();
            dirs.extend(versions.into_iter().rev()); // newest Node first
        }
    }
    if cfg!(target_os = "macos") {
        dirs.push("/opt/homebrew/bin".into());
        dirs.push("/usr/local/bin".into());
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            dirs.push(PathBuf::from(appdata).join("npm"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(&local).join("Programs").join(name));
            dirs.push(PathBuf::from(local).join("Microsoft").join("WinGet").join("Links"));
        }
    }
    dirs.iter().flat_map(|d| exe_names.iter().map(move |n| d.join(n))).collect()
}

fn find(name: &str) -> Option<PathBuf> {
    candidates(name).into_iter().find(|p| p.is_file())
}

/// PATH for a CLI child process: its own folder first (npm CLIs need the `node` next to
/// them), then the usual places, then whatever we inherited.
fn child_path(bin: &Path) -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = bin.parent() {
        dirs.push(d.to_path_buf());
    }
    if cfg!(target_os = "macos") {
        dirs.push("/opt/homebrew/bin".into());
        dirs.push("/usr/local/bin".into());
    }
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

fn command(bin: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(bin);
    cmd.env("PATH", child_path(bin)).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

async fn run_short(bin: &Path, args: &[&str]) -> Option<(bool, String)> {
    let out = tokio::time::timeout(Duration::from_secs(15), command(bin).args(args).output()).await.ok()?.ok()?;
    Some((out.status.success(), String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

async fn detect_claude_code() -> Option<CliInfo> {
    let path = find("claude")?;
    let (ok, stdout) = run_short(&path, &["auth", "status", "--json"]).await.unwrap_or((false, String::new()));
    let status: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    let logged_in = ok && status["loggedIn"].as_bool().unwrap_or(false);
    let detail = match (status["authMethod"].as_str(), status["subscriptionType"].as_str()) {
        (Some("claude.ai"), Some(plan)) => Some(format!("Claude {}", capitalize(plan))),
        (Some("claude.ai"), None) => Some("Claude-prenumeration".into()),
        (Some(_), _) => Some("Inloggad med API-nyckel".into()),
        _ => None,
    };
    Some(CliInfo { path, logged_in, detail })
}

async fn detect_codex() -> Option<CliInfo> {
    let path = find("codex")?;
    // `codex login status` exits 0 when signed in and prints how.
    let (ok, stdout) = run_short(&path, &["login", "status"]).await.unwrap_or((false, String::new()));
    let detail = stdout.lines().next().map(str::to_string).filter(|s| !s.is_empty());
    Some(CliInfo { path, logged_in: ok, detail })
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

pub async fn detect() -> Detected {
    let (claude_code, codex) = tokio::join!(detect_claude_code(), detect_codex());
    Detected { claude_code, codex }
}

// ---------- running ----------

const CLI_TIMEOUT: Duration = Duration::from_secs(600);

/// The CLIs take a single prompt, so earlier turns are written out as text.
pub fn flatten(prompt: &Prompt) -> String {
    let mut out = String::new();
    if let Some(ctx) = &prompt.context {
        out.push_str(ctx);
        out.push_str("\n\n");
    }
    let n = prompt.turns.len();
    if n > 1 {
        out.push_str("Tidigare i den här konversationen:\n\n");
        for t in &prompt.turns[..n - 1] {
            let who = if t.role == Role::User { "Säljaren" } else { "Du" };
            out.push_str(&format!("{who}: {}\n\n", t.text.trim()));
        }
        out.push_str("Ny fråga från säljaren:\n");
    }
    if let Some(last) = prompt.turns.last() {
        out.push_str(&last.text);
    }
    out
}

async fn run_cli(mut cmd: tokio::process::Command, stdin_text: String, what: &str) -> Result<(String, String), String> {
    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Kunde inte starta {what}: {e}"))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let writer = tokio::spawn(async move {
        let _ = stdin.write_all(stdin_text.as_bytes()).await;
        drop(stdin);
    });
    let out = tokio::time::timeout(CLI_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| format!("{what} svarade inte inom tio minuter."))?
        .map_err(|e| format!("{what} avbröts: {e}"))?;
    let _ = writer.await;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !out.status.success() && stdout.trim().is_empty() {
        let first = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("okänt fel");
        return Err(format!("{what} gav ett fel: {first}"));
    }
    Ok((stdout, stderr))
}

async fn run_claude_code(path: &Path, model: &str, prompt: &Prompt) -> Result<String, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut cmd = command(path);
    cmd.current_dir(dir.path()).args([
        "-p",
        "--output-format",
        "json",
        "--tools",
        "",
        "--strict-mcp-config",
        "--setting-sources",
        "",
        "--disable-slash-commands",
        "--no-session-persistence",
        "--model",
        model,
        "--system-prompt",
        &prompt.system,
    ]);
    let schema;
    if let Some(s) = &prompt.schema {
        schema = s.to_string();
        cmd.args(["--json-schema", &schema]);
    }
    let (stdout, _) = run_cli(cmd, flatten(prompt), "Claude Code").await?;
    let v: Value = serde_json::from_str(stdout.trim()).map_err(|_| {
        format!("Oväntat svar från Claude Code: {}", stdout.chars().take(200).collect::<String>())
    })?;
    let text = v["result"].as_str().unwrap_or_default().to_string();
    if v["is_error"].as_bool().unwrap_or(false) {
        return Err(format!("Claude Code gav ett fel: {text}"));
    }
    if prompt.schema.is_some() {
        if let Some(structured) = v.get("structured_output").filter(|s| !s.is_null()) {
            return Ok(structured.to_string());
        }
    }
    if text.trim().is_empty() {
        return Err("Claude Code returnerade inget svar.".into());
    }
    Ok(text)
}

async fn run_codex(path: &Path, prompt: &Prompt) -> Result<String, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let work = dir.path().join("work");
    std::fs::create_dir(&work).map_err(|e| e.to_string())?;
    let out_file = dir.path().join("answer.txt");
    let mut cmd = command(path);
    cmd.current_dir(&work).args(["exec", "--skip-git-repo-check", "--ephemeral", "--sandbox", "read-only", "--ignore-user-config"]);
    cmd.arg("--output-last-message").arg(&out_file);
    if let Some(s) = &prompt.schema {
        let schema_file = dir.path().join("schema.json");
        std::fs::write(&schema_file, s.to_string()).map_err(|e| e.to_string())?;
        cmd.arg("--output-schema").arg(&schema_file);
    }
    cmd.arg("-");
    // Codex has no separate system prompt flag; the instructions go first.
    let text = format!("{}\n\n{}", prompt.system, flatten(prompt));
    let (stdout, _) = run_cli(cmd, text, "Codex").await?;
    let answer = std::fs::read_to_string(&out_file).unwrap_or(stdout);
    if answer.trim().is_empty() {
        return Err("Codex returnerade inget svar.".into());
    }
    Ok(answer.trim().to_string())
}

pub async fn complete(backend: &Backend, prompt: &Prompt) -> Result<String, String> {
    match backend {
        Backend::Api { key, model } => crate::claude::create_message(key, model, prompt).await,
        Backend::ClaudeCode { path, model } => run_claude_code(path, model, prompt).await,
        Backend::Codex { path } => run_codex(path, prompt).await,
    }
}

// ---------- tips ----------

/// Selling points. Retries once if the answer isn't valid JSON for our schema, then drops
/// every point whose quote isn't in the transcript.
pub async fn generate_tips(backend: &Backend, segments: &[Segment], stereo: bool, profile: &Profile) -> Result<Tips, String> {
    if segments.is_empty() {
        return Err("Transkriberingen är tom, det finns inget att analysera.".into());
    }
    let mut prompt = Prompt {
        system: format!("{}\n\n{}", tips::SYSTEM_PROMPT, sales::PLAYBOOK),
        context: None,
        turns: vec![Turn { role: Role::User, text: tips::user_prompt(segments, stereo, profile) }],
        schema: Some(tips::response_schema()),
        max_tokens: 16_000,
    };
    let first = complete(backend, &prompt).await?;
    let parsed = match tips::parse_and_validate(&first, segments.len()) {
        Ok(t) => t,
        Err(why) => {
            log::warn!("tips: invalid first answer ({why}), retrying once");
            prompt.turns[0].text.push_str(&format!(
                "\n\nObs: ett tidigare svar underkändes ({why}). Svara med enbart giltig JSON enligt schemat."
            ));
            let second = complete(backend, &prompt).await?;
            tips::parse_and_validate(&second, segments.len())
                .map_err(|e| format!("AI:n svarade inte med giltig JSON, inte ens vid andra försöket ({e})."))?
        }
    };
    Ok(tips::verify_against_transcript(parsed, segments))
}

// ---------- chat ----------

pub const CHAT_SYSTEM_PROMPT: &str = "\
Du är en erfaren svensk säljcoach. Säljaren har spelat in ett säljsamtal och vill prata om det \
med dig. Du har transkriberingen av samtalet och, om de finns, säljtips som redan tagits fram.

- Svara på svenska, kort och konkret: oftast 2–6 meningar eller en kort punktlista.
- Hänvisa till ställen i samtalet med tidsstämpel i formatet [mm:ss]. Citera ordagrant.
- När säljaren frågar vad hen kunde ha sagt istället: ge konkreta formuleringar att använda.
- Utgå bara från samtalet. Hitta inte på vad som sades; säg till om något inte framgår.
- Transkriberingen är automatisk och kan innehålla felhörningar.";

/// The call as context for chat: transcript, speaker roles and earlier tips.
pub fn chat_context(title: &str, segments: &[Segment], tips: Option<&Tips>, profile: &Profile) -> String {
    let mut out = format!("Samtal: {title}\n\n");
    if let Some(p) = profile.prompt_block() {
        out.push_str(&p);
        out.push_str("\n\n");
    }
    if let Some(t) = tips {
        let roles: Vec<String> = t.channel_roles.iter().map(|r| format!("{} = {}", r.speaker, r.role)).collect();
        if !roles.is_empty() {
            out.push_str(&format!("Talare: {}\n\n", roles.join(", ")));
        }
        if !t.customer_name.is_empty() || !t.customer_company.is_empty() {
            let who = [t.customer_name.as_str(), t.customer_company.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>();
            out.push_str(&format!("Kunden: {}\n\n", who.join(", ")));
        }
    }
    out.push_str("<transkribering>\n");
    out.push_str(&format_for_model(segments));
    out.push_str("\n</transkribering>\n");
    if let Some(t) = tips {
        out.push_str("\n<säljtips>\n");
        out.push_str(&tips::format_tips_plain(t, title));
        out.push_str("\n</säljtips>\n");
    }
    out
}

pub fn chat_prompt(context: String, history: &[Turn], question: &str) -> Prompt {
    let mut turns: Vec<Turn> = history.to_vec();
    turns.push(Turn { role: Role::User, text: question.trim().to_string() });
    Prompt {
        system: format!("{CHAT_SYSTEM_PROMPT}\n\n{}", sales::PLAYBOOK),
        context: Some(context),
        turns,
        schema: None,
        max_tokens: 4_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(turns: &[(&str, Role)]) -> Prompt {
        Prompt {
            system: "S".into(),
            context: Some("<transkribering>…</transkribering>".into()),
            turns: turns.iter().map(|(t, r)| Turn { role: *r, text: t.to_string() }).collect(),
            schema: None,
            max_tokens: 100,
        }
    }

    #[test]
    fn flatten_single_question() {
        let p = prompt(&[("Hur gick det?", Role::User)]);
        assert_eq!(flatten(&p), "<transkribering>…</transkribering>\n\nHur gick det?");
    }

    #[test]
    fn flatten_keeps_earlier_turns_in_order() {
        let p = prompt(&[("Hur gick det?", Role::User), ("Bra.", Role::Assistant), ("Vad kunde jag sagt?", Role::User)]);
        let text = flatten(&p);
        let a = text.find("Säljaren: Hur gick det?").unwrap();
        let b = text.find("Du: Bra.").unwrap();
        let c = text.find("Ny fråga från säljaren:\nVad kunde jag sagt?").unwrap();
        assert!(a < b && b < c, "{text}");
    }

    #[test]
    fn provider_ids_round_trip() {
        for p in [Provider::Api, Provider::ClaudeCode, Provider::Codex] {
            assert_eq!(Provider::parse(p.as_str()), Some(p));
        }
        assert_eq!(Provider::parse("gemini"), None);
    }

    #[test]
    fn chat_context_includes_transcript_roles_and_tips() {
        let segs = vec![Segment { id: 0, start_ms: 0, end_ms: 1_000, text: "Hej!".into(), speaker: Some("Talare 1".into()) }];
        let tips: Tips = serde_json::from_value(serde_json::json!({
            "speakers": [], "channelRoles": [{"speaker": "Talare 1", "role": "Säljare"}],
            "summary": "Kort samtal.", "sellingPoints": [], "customerNeeds": [], "objections": [],
            "missedOpportunities": [], "nextSteps": ["Ring igen"]
        }))
        .unwrap();
        let profile = Profile { name: "Anna".into(), ..Default::default() };
        let ctx = chat_context("Demo", &segs, Some(&tips), &profile);
        assert!(ctx.contains("Säljaren heter Anna"));
        assert!(ctx.contains("Talare: Talare 1 = Säljare"));
        assert!(ctx.contains("#0 [00:00] Talare 1: Hej!"));
        assert!(ctx.contains("Nästa steg\n• Ring igen"));
        assert!(!chat_context("Demo", &segs, None, &Profile::default()).contains("<säljtips>"));
    }

    #[test]
    fn candidates_include_common_install_dirs() {
        let c = candidates("claude");
        assert!(c.iter().any(|p| p.ends_with(if cfg!(windows) { ".local/bin/claude.exe" } else { ".local/bin/claude" })));
    }
}
