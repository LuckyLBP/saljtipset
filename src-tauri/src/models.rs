//! KB-Whisper models: catalog, hardware-based recommendation, and resumable download.
//!
//! File names come from the KBLab repos' file listings (checked 2026-10-27):
//! each of kb-whisper-{large,medium,small} has `ggml-model.bin` (f16) and
//! `ggml-model-q5_0.bin`. We use the q5_0 files. Size and SHA256 are read from the
//! Hugging Face API at download time, so a re-uploaded file is still verified correctly.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelId {
    Bast,
    Snabb,
    Latt,
}

pub struct ModelSpec {
    pub id: ModelId,
    pub label: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub approx_bytes: u64,
    /// Rough minutes to transcribe a 30-minute call, with a GPU and on CPU only.
    /// "Bäst" on GPU is measured (M4 Pro, ~4x realtime with beam search); the rest are estimates.
    pub minutes_gpu: &'static str,
    pub minutes_cpu: &'static str,
    pub description: &'static str,
}

pub const CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: ModelId::Bast,
        label: "Bäst",
        repo: "KBLab/kb-whisper-large",
        file: "ggml-model-q5_0.bin",
        approx_bytes: 1_081_140_203,
        minutes_gpu: "6–8",
        minutes_cpu: "40–60",
        description: "Högst träffsäkerhet. Kräver grafikkort eller Apple Silicon.",
    },
    ModelSpec {
        id: ModelId::Snabb,
        label: "Snabb",
        repo: "KBLab/kb-whisper-medium",
        file: "ggml-model-q5_0.bin",
        approx_bytes: 539_212_484,
        minutes_gpu: "3–5",
        minutes_cpu: "20–30",
        description: "Nästan lika bra, ungefär dubbelt så snabb.",
    },
    ModelSpec {
        id: ModelId::Latt,
        label: "Lätt",
        repo: "KBLab/kb-whisper-small",
        file: "ggml-model-q5_0.bin",
        approx_bytes: 175_209_680,
        minutes_gpu: "1–2",
        minutes_cpu: "6–10",
        description: "Minst och snabbast. Bra på äldre datorer utan grafikkort.",
    },
];

pub fn spec(id: ModelId) -> &'static ModelSpec {
    CATALOG.iter().find(|m| m.id == id).expect("every id is in the catalog")
}

impl ModelId {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelId::Bast => "bast",
            ModelId::Snabb => "snabb",
            ModelId::Latt => "latt",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        CATALOG.iter().map(|m| m.id).find(|id| id.as_str() == s)
    }
}

pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("models")
}

/// Where a model lives on disk once downloaded and verified.
pub fn model_path(app_data: &Path, id: ModelId) -> PathBuf {
    let s = spec(id);
    let repo_name = s.repo.rsplit('/').next().unwrap();
    models_dir(app_data).join(format!("{repo_name}-{}", s.file))
}

fn part_path(final_path: &Path) -> PathBuf {
    final_path.with_extension("bin.part")
}

pub fn is_installed(app_data: &Path, id: ModelId) -> bool {
    model_path(app_data, id).is_file()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub gpu: bool,
    pub gpu_name: Option<String>,
    pub total_memory_gb: f32,
    pub cpu_cores: usize,
    pub recommended: ModelId,
}

/// Recommend "Bäst" when there's a usable GPU (or Apple Silicon), otherwise "Lätt".
pub fn detect_hardware(gpu_enabled: bool) -> Hardware {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let total_memory_gb = sys.total_memory() as f32 / 1_073_741_824.0;
    let cpu_cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let (gpu, gpu_name) = if gpu_enabled { crate::gpu::probe() } else { (false, None) };
    let recommended = if gpu && total_memory_gb >= 7.5 { ModelId::Bast } else { ModelId::Latt };
    Hardware { gpu, gpu_name, total_memory_gb, cpu_cores, recommended }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model: ModelId,
    pub downloaded: u64,
    pub total: u64,
    pub bytes_per_sec: u64,
    pub phase: &'static str,
}

struct RemoteFile {
    size: u64,
    sha256: String,
}

/// Size and SHA256 of the file according to Hugging Face's metadata.
async fn remote_metadata(client: &reqwest::Client, s: &ModelSpec) -> Result<RemoteFile, String> {
    let url = format!("https://huggingface.co/api/models/{}/tree/main", s.repo);
    let entries: Vec<serde_json::Value> = client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("Kunde inte hämta modellinformation från Hugging Face: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Oväntat svar från Hugging Face: {e}"))?;
    let entry = entries
        .iter()
        .find(|e| e["path"] == s.file)
        .ok_or_else(|| format!("Filen {} finns inte längre i {}.", s.file, s.repo))?;
    let size = entry["lfs"]["size"].as_u64().or_else(|| entry["size"].as_u64()).ok_or("Storlek saknas i metadata")?;
    let sha256 = entry["lfs"]["oid"].as_str().ok_or("SHA256 saknas i metadata")?.to_lowercase();
    Ok(RemoteFile { size, sha256 })
}

async fn hash_existing(path: &Path, hasher: &mut Sha256) -> std::io::Result<u64> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut buf = vec![0u8; 1 << 20];
    let mut total = 0;
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            return Ok(total);
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
}

/// Download a model into the app data dir. Resumes a previous `.part` file, verifies the
/// SHA256 against Hugging Face's metadata and only then moves it into place.
pub async fn download(
    app_data: &Path,
    id: ModelId,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(DownloadProgress),
) -> Result<PathBuf, String> {
    let s = spec(id);
    let final_path = model_path(app_data, id);
    let part = part_path(&final_path);
    tokio::fs::create_dir_all(models_dir(app_data)).await.map_err(|e| e.to_string())?;

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let remote = remote_metadata(&client, s).await?;

    // Resume: hash what we already have so the final checksum covers the whole file.
    let mut hasher = Sha256::new();
    let mut have = if part.is_file() {
        on_progress(DownloadProgress { model: id, downloaded: 0, total: remote.size, bytes_per_sec: 0, phase: "verifying" });
        hash_existing(&part, &mut hasher).await.map_err(|e| e.to_string())?
    } else {
        0
    };
    if have > remote.size {
        // Leftover from a different file version.
        let _ = tokio::fs::remove_file(&part).await;
        hasher = Sha256::new();
        have = 0;
    }

    if have < remote.size {
        let url = format!("https://huggingface.co/{}/resolve/main/{}", s.repo, s.file);
        let mut req = client.get(&url);
        if have > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let resp = req
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("Nedladdningen kunde inte starta: {e}"))?;
        if have > 0 && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            // Server ignored the range: start over.
            hasher = Sha256::new();
            have = 0;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(have > 0)
            .truncate(have == 0)
            .open(&part)
            .await
            .map_err(|e| e.to_string())?;

        let mut stream = resp.bytes_stream();
        let started = Instant::now();
        let start_bytes = have;
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        while let Some(chunk) = stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                file.flush().await.ok();
                return Err("Avbrutet".into());
            }
            let chunk = chunk.map_err(|e| format!("Nedladdningen avbröts: {e}. Starta igen så fortsätter den där den slutade."))?;
            file.write_all(&chunk).await.map_err(|e| format!("Kunde inte skriva till disken: {e}"))?;
            hasher.update(&chunk);
            have += chunk.len() as u64;
            if last_emit.elapsed() >= Duration::from_millis(200) {
                last_emit = Instant::now();
                let secs = started.elapsed().as_secs_f64().max(0.001);
                on_progress(DownloadProgress {
                    model: id,
                    downloaded: have,
                    total: remote.size,
                    bytes_per_sec: ((have - start_bytes) as f64 / secs) as u64,
                    phase: "downloading",
                });
            }
        }
        file.flush().await.map_err(|e| e.to_string())?;
        file.sync_all().await.ok();
    }

    on_progress(DownloadProgress { model: id, downloaded: have, total: remote.size, bytes_per_sec: 0, phase: "verifying" });
    let digest = hex::encode(hasher.finalize());
    if have != remote.size || digest != remote.sha256 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err("Den nedladdade filen var skadad (kontrollsumman stämde inte). Försök igen.".into());
    }
    tokio::fs::rename(&part, &final_path).await.map_err(|e| e.to_string())?;
    on_progress(DownloadProgress { model: id, downloaded: have, total: remote.size, bytes_per_sec: 0, phase: "done" });
    Ok(final_path)
}

/// Remove a model and any half-finished download of it.
pub fn delete(app_data: &Path, id: ModelId) -> std::io::Result<()> {
    let p = model_path(app_data, id);
    for path in [part_path(&p), p] {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for m in CATALOG {
            assert_eq!(ModelId::parse(m.id.as_str()), Some(m.id));
        }
        assert_eq!(ModelId::parse("jättestor"), None);
    }

    #[test]
    fn model_paths_are_distinct() {
        let base = Path::new("/data");
        let paths: Vec<_> = CATALOG.iter().map(|m| model_path(base, m.id)).collect();
        assert_eq!(paths[0], Path::new("/data/models/kb-whisper-large-ggml-model-q5_0.bin"));
        assert_ne!(paths[1], paths[2]);
        assert_eq!(part_path(&paths[0]), Path::new("/data/models/kb-whisper-large-ggml-model-q5_0.bin.part"));
    }
}
