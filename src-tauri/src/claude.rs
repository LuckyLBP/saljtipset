//! Anthropic Messages API over plain HTTP. Only transcript text is ever sent.

use std::time::Duration;

use serde_json::{Value, json};

use crate::ai::Prompt;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const API_VERSION: &str = "2026-10-21";
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";

/// Models that accept server-side refusal fallbacks (`fallbacks: "default"`).
const FALLBACK_MODELS: &[&str] = &["claude-sonnet-5-5", "claude-opus-5-5", "claude-opus-5", "claude-fable-5-1"];
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .expect("http client")
}

/// Turn an API error response into a Swedish message the user can act on.
fn api_error(status: reqwest::StatusCode, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(300).collect());
    match status.as_u16() {
        401 => "API-nyckeln godkändes inte. Kontrollera nyckeln under Inställningar.".into(),
        403 => format!("Nyckeln saknar behörighet: {detail}"),
        404 => format!("Modellen hittades inte. Kontrollera modellnamnet under Inställningar. ({detail})"),
        429 => "Anthropic begränsar antalet anrop just nu. Vänta en stund och försök igen.".into(),
        529 | 500..=599 => "Anthropics tjänst är tillfälligt överbelastad. Försök igen om en stund.".into(),
        _ => format!("Anropet till Claude misslyckades ({status}): {detail}"),
    }
}

fn network_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "Claude svarade inte i tid. Försök igen.".into()
    } else if e.is_connect() {
        "Kunde inte nå Anthropic. Kontrollera internetanslutningen.".into()
    } else {
        format!("Nätverksfel: {e}")
    }
}

/// "Testa nyckeln": a cheap authenticated call that also checks the configured model exists.
pub async fn test_key(api_key: &str, model: &str) -> Result<(), String> {
    let resp = client()
        .get(format!("{MODELS_URL}/{model}"))
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .send()
        .await
        .map_err(network_error)?;
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let body = resp.text().await.unwrap_or_default();
    Err(api_error(status, &body))
}

/// One Messages call; returns the concatenated text blocks.
pub async fn create_message(api_key: &str, model: &str, prompt: &Prompt) -> Result<String, String> {
    // The transcript goes first, marked for caching, so follow-up chat questions about the
    // same call don't pay for it again.
    let mut messages: Vec<Value> = Vec::new();
    for (i, turn) in prompt.turns.iter().enumerate() {
        let mut content = Vec::new();
        if i == 0 {
            if let Some(ctx) = &prompt.context {
                content.push(json!({"type": "text", "text": ctx, "cache_control": {"type": "ephemeral"}}));
            }
        }
        content.push(json!({"type": "text", "text": turn.text}));
        messages.push(json!({"role": turn.role.as_str(), "content": content}));
    }
    let mut body = json!({
        "model": model,
        "max_tokens": prompt.max_tokens,
        "system": prompt.system,
        "messages": messages,
    });
    if let Some(schema) = &prompt.schema {
        body["output_config"] = json!({"format": {"type": "json_schema", "schema": schema}});
    }
    let mut req = client()
        .post(API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json");
    if FALLBACK_MODELS.contains(&model) {
        // If a safety classifier declines, let the API re-run on its recommended fallback model.
        body["fallbacks"] = json!("default");
        req = req.header("anthropic-beta", FALLBACK_BETA);
    }

    let resp = req.json(&body).send().await.map_err(network_error)?;
    let status = resp.status();
    let text = resp.text().await.map_err(network_error)?;
    if !status.is_success() {
        return Err(api_error(status, &text));
    }
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("Oväntat svar från Claude: {e}"))?;
    match v["stop_reason"].as_str() {
        Some("refusal") => return Err("Claude avböjde att svara.".into()),
        Some("max_tokens") => return Err("Svaret blev för långt och klipptes av.".into()),
        _ => {}
    }
    let out: String = v["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    if out.trim().is_empty() {
        return Err("Claude returnerade inget svar.".into());
    }
    Ok(out)
}
