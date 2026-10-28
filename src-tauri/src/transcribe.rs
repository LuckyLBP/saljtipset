//! Local transcription with whisper.cpp. Runs on the calling thread; the app spawns one.
//!
//! Language is fixed to Swedish. Stereo calls with two real sides are transcribed one
//! channel at a time and merged by timestamp ("Talare 1" = left, "Talare 2" = right).
//! Each channel is cut into speech turns (vad.rs) and whisper runs once per turn, walking
//! long turns in its own 30-second windows. Beyond the model itself, memory is the 16 kHz
//! samples (~230 MB per channel per hour) plus one turn's working buffers. Measured: a 66-minute
//! stereo call with "Bäst" on an M4 Pro peaked at 2.7 GB in total, 1.1 GB of it the model.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio::{self, AudioError, Decoded};
use crate::vad;
use crate::transcript::{Segment, clean_text, merge_channels, speaker_for_channel};

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Decoding,
    Loading,
    Transcribing,
}

pub trait Sink {
    /// Overall progress 0..=100 within the current phase.
    fn progress(&self, phase: Phase, percent: f32);
    /// A finished segment, as soon as whisper produces it. `id` is provisional until done.
    fn segment(&self, segment: &Segment);
}

pub struct Options {
    pub use_gpu: bool,
    pub threads: usize,
}

impl Default for Options {
    fn default() -> Self {
        // Physical cores, not hyperthreads: ggml's threads wait for each other, so extra
        // logical cores (or slow efficiency cores) make it slower, not faster.
        let cores = sysinfo::System::physical_core_count()
            .or_else(|| std::thread::available_parallelism().ok().map(|n| n.get()))
            .unwrap_or(4);
        Self { use_gpu: crate::gpu::compiled_with_gpu(), threads: cores.clamp(1, MAX_THREADS) }
    }
}

/// On an M4 Pro (CPU only), "Bäst" ran 20% faster on 8 threads than 4; "Lätt" was a bit
/// faster on 4. Physical cores up to 8 suits the bigger models people pick.
const MAX_THREADS: usize = 8;

pub struct Output {
    pub segments: Vec<Segment>,
    pub duration_ms: u64,
    pub channels: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscribeError {
    #[error(transparent)]
    Audio(#[from] AudioError),
    #[error("Talmodellen kunde inte laddas. Ladda ner den igen under Inställningar.")]
    ModelLoad,
    #[error("Transkriberingen misslyckades: {0}")]
    Whisper(String),
    #[error(
        "Grafikdrivrutinen (Vulkan) saknas, så transkriberingen kan inte starta. Installera om Bubbe eller uppdatera grafikdrivrutinen."
    )]
    VulkanMissing,
    #[error("Avbrutet")]
    Cancelled,
}

/// Keeps the loaded model between calls; loading `large` takes a few seconds.
#[derive(Default)]
pub struct ContextCache {
    inner: Mutex<Option<(PathBuf, bool, Arc<WhisperContext>)>>,
}

impl ContextCache {
    pub fn get(&self, model: &Path, use_gpu: bool) -> Result<Arc<WhisperContext>, TranscribeError> {
        let mut guard = self.inner.lock().unwrap();
        if let Some((p, g, ctx)) = guard.as_ref() {
            if p == model && *g == use_gpu {
                return Ok(ctx.clone());
            }
        }
        *guard = None; // free the old model before loading a new one
        let mut params = WhisperContextParameters::default();
        params.use_gpu(use_gpu);
        params.flash_attn(use_gpu);
        let path = model.to_str().ok_or(TranscribeError::ModelLoad)?;
        let ctx = crate::gpu::guarded(|| WhisperContext::new_with_params(path, params))
            .map_err(|_| TranscribeError::ModelLoad)?;
        let ctx = Arc::new(ctx);
        *guard = Some((model.to_path_buf(), use_gpu, ctx.clone()));
        Ok(ctx)
    }

    pub fn clear(&self) {
        *self.inner.lock().unwrap() = None;
    }
}

/// whisper.cpp polls this between steps; returning true stops `whisper_full`.
unsafe extern "C" fn abort_trampoline(user_data: *mut c_void) -> bool {
    // SAFETY: points at the AtomicBool owned by `transcribe_file`, which outlives the call.
    unsafe { (*(user_data as *const AtomicBool)).load(Ordering::Relaxed) }
}

pub fn transcribe_file(
    cache: &ContextCache,
    model: &Path,
    audio_path: &Path,
    opts: &Options,
    cancel: Arc<AtomicBool>,
    sink: Arc<dyn Sink + Send + Sync>,
) -> Result<Output, TranscribeError> {
    if cfg!(feature = "gpu-vulkan") && !crate::gpu::vulkan_loader_present() {
        return Err(TranscribeError::VulkanMissing);
    }

    let decoded: Decoded = audio::decode_file(audio_path, &cancel, |p| sink.progress(Phase::Decoding, p * 100.0))?;
    let duration_ms = decoded.duration_ms();
    let n_channels = decoded.channels.len();

    sink.progress(Phase::Loading, 0.0);
    let ctx = cache.get(model, opts.use_gpu)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(TranscribeError::Cancelled);
    }
    // A Vulkan build on a PC without a usable GPU runs on the CPU; plan for what whisper got.
    let opts = &Options { use_gpu: opts.use_gpu && crate::gpu::device_found(), threads: opts.threads };

    let mut per_channel = Vec::with_capacity(n_channels);
    for (ch, samples) in decoded.channels.into_iter().enumerate() {
        let segs = transcribe_channel(&ctx, &samples, ch, n_channels, opts, &cancel, &sink)?;
        drop(samples);
        per_channel.push(segs);
    }
    sink.progress(Phase::Transcribing, 100.0);
    Ok(Output { segments: merge_channels(per_channel), duration_ms, channels: n_channels })
}

fn transcribe_channel(
    ctx: &WhisperContext,
    samples: &[f32],
    channel: usize,
    n_channels: usize,
    opts: &Options,
    cancel: &Arc<AtomicBool>,
    sink: &Arc<dyn Sink + Send + Sync>,
) -> Result<Vec<Segment>, TranscribeError> {
    // One whisper run per speech turn (see vad.rs). No speech, no segments.
    let turns = vad::speech_turns(samples);
    let total: usize = turns.iter().map(|(s, e)| e - s).sum();
    let speaker = speaker_for_channel(channel, n_channels);
    // One state for the whole channel: whisper carries the previous text over as context.
    let mut state = ctx.create_state().map_err(|e| TranscribeError::Whisper(e.to_string()))?;
    let mut segments: Vec<Segment> = Vec::new();
    let mut done = 0usize;

    for (start, end) in turns {
        let offset_ms = vad::samples_to_ms(start);
        // whisper.cpp ignores input under one second; pad very short turns with silence.
        let min = vad::ms_to_samples(1_100);
        let padded;
        let chunk: &[f32] = if end - start < min {
            padded = [&samples[start..end], &vec![0.0; min - (end - start)][..]].concat();
            &padded
        } else {
            &samples[start..end]
        };

        let mut params = base_params(opts);
        {
            let sink = sink.clone();
            let speaker = speaker.clone();
            let mut n = segments.len();
            params.set_segment_callback_safe_lossy(move |data: whisper_rs::SegmentCallbackData| {
                let Some(text) = clean_text(&data.text) else { return };
                sink.segment(&Segment {
                    id: n,
                    start_ms: offset_ms + data.start_timestamp.max(0) as u64 * 10,
                    end_ms: offset_ms + data.end_timestamp.max(0) as u64 * 10,
                    text,
                    speaker: speaker.clone(),
                });
                n += 1;
            });
        }
        {
            let sink = sink.clone();
            let (len, before) = ((end - start) as f32, done as f32);
            params.set_progress_callback_safe(move |p: i32| {
                let in_channel = (before + len * p as f32 / 100.0) / total.max(1) as f32;
                let overall = (channel as f32 + in_channel) * 100.0 / n_channels as f32;
                sink.progress(Phase::Transcribing, overall.min(99.0));
            });
        }
        // SAFETY: `cancel` is an Arc held by our caller for the whole `full` call.
        unsafe {
            params.set_abort_callback(Some(abort_trampoline));
            params.set_abort_callback_user_data(Arc::as_ptr(cancel) as *mut c_void);
        }

        let result = state.full(params, chunk);
        if cancel.load(Ordering::Relaxed) {
            return Err(TranscribeError::Cancelled);
        }
        result.map_err(|e| TranscribeError::Whisper(e.to_string()))?;

        let turn_end_ms = vad::samples_to_ms(end);
        for seg in state.as_iter() {
            let Some(text) = seg.to_str_lossy().ok().and_then(|t| clean_text(&t)) else { continue };
            segments.push(Segment {
                id: segments.len(),
                start_ms: offset_ms + seg.start_timestamp().max(0) as u64 * 10,
                // Padding can push whisper's end past the real turn.
                end_ms: (offset_ms + seg.end_timestamp().max(0) as u64 * 10).min(turn_end_ms),
                text,
                speaker: speaker.clone(),
            });
        }
        done += end - start;
    }
    Ok(segments)
}

fn base_params(opts: &Options) -> FullParams<'static, 'static> {
    // Beam search is noticeably better on Swedish but ~2x slower; only worth it on a GPU.
    let strategy = if opts.use_gpu {
        SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 }
    } else {
        SamplingStrategy::Greedy { best_of: 1 }
    };
    let mut params = FullParams::new(strategy);
    params.set_language(Some("sv"));
    params.set_translate(false);
    params.set_n_threads(opts.threads as i32);
    params.set_no_timestamps(false);
    params.set_no_context(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_nst(true);
    params
}
