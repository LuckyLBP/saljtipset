//! Audio decoding: any supported file -> 16 kHz f32 channels, which is what whisper wants.
//!
//! Decoding is streamed packet by packet and resampled on the fly, so we never hold the file
//! at its original sample rate in memory. A 60-minute stereo call ends up as two 16 kHz
//! buffers of ~230 MB each.
//!
//! Symphonia handles wav, mp3, m4a/mp4/aac, ogg/vorbis, flac. Anything it can't read
//! (typically webm/opus) goes through ffmpeg if one is available, see [`find_ffmpeg`].

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

pub const TARGET_RATE: u32 = 16_000;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("Filen kunde inte öppnas: {0}")]
    Open(String),
    #[error(
        "Ljudformatet stöds inte. Prova att spara samtalet som mp3, m4a eller wav, eller installera ffmpeg. ({0})"
    )]
    Unsupported(String),
    #[error("Filen innehåller inget ljud.")]
    Empty,
    #[error("Avbrutet")]
    Cancelled,
}

/// Decoded audio at 16 kHz. One channel for mono, two when the call has two distinct sides.
pub struct Decoded {
    pub channels: Vec<Vec<f32>>,
    /// Channel count in the source file, before we decided mono vs. stereo.
    pub source_channels: usize,
}

impl Decoded {
    pub fn duration_ms(&self) -> u64 {
        self.channels.first().map_or(0, |c| c.len() as u64 * 1000 / TARGET_RATE as u64)
    }
}

/// Decode `path` to 16 kHz. `progress` gets 0.0..=1.0 when the duration is known.
pub fn decode_file(
    path: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f32),
) -> Result<Decoded, AudioError> {
    let raw = match decode_symphonia(path, cancel, &mut progress) {
        Ok(raw) => raw,
        Err(AudioError::Unsupported(why)) => match find_ffmpeg() {
            Some(ffmpeg) => decode_ffmpeg(&ffmpeg, path, cancel)?,
            None => return Err(AudioError::Unsupported(why)),
        },
        Err(e) => return Err(e),
    };
    progress(1.0);
    Ok(finalize_channels(raw))
}

/// Resampled channels straight from the decoder, before the mono/stereo decision.
struct Raw {
    channels: Vec<Vec<f32>>,
}

fn decode_symphonia(
    path: &Path,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(f32),
) -> Result<Raw, AudioError> {
    let file = File::open(path).map_err(|e| AudioError::Open(e.to_string()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| AudioError::Unsupported(e.to_string()))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| AudioError::Unsupported("inget ljudspår".into()))?;
    let track_id = track.id;
    let total_frames = track.num_frames;
    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| AudioError::Unsupported("okänd kodek".into()))?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
        .map_err(|e| AudioError::Unsupported(e.to_string()))?;

    let mut resamplers: Vec<Resampler> = Vec::new();
    let mut out: Vec<Vec<f32>> = Vec::new();
    let mut planar: Vec<Vec<f32>> = Vec::new();
    let mut frames_done: u64 = 0;
    let mut last_report = 0.0f32;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AudioError::Cancelled);
        }
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymError::ResetRequired) => break,
            // A truncated file (common for recordings that were cut off) still has usable audio.
            Err(SymError::IoError(_)) => break,
            Err(e) => return Err(AudioError::Unsupported(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
            Err(e) => return Err(AudioError::Unsupported(e.to_string())),
        };
        let rate = buf.spec().rate();
        buf.copy_to_vecs_planar::<f32>(&mut planar);
        frames_done += buf.frames() as u64;

        // More than two channels (rare for calls): fold everything to mono.
        let input: Vec<&[f32]> = if planar.len() > 2 {
            let n = planar[0].len();
            let mut mono = vec![0.0f32; n];
            for ch in &planar {
                for (m, s) in mono.iter_mut().zip(ch) {
                    *m += *s / planar.len() as f32;
                }
            }
            planar.truncate(1);
            planar[0] = mono;
            vec![&planar[0]]
        } else {
            planar.iter().map(|c| c.as_slice()).collect()
        };

        if resamplers.is_empty() {
            resamplers = input.iter().map(|_| Resampler::new(rate, TARGET_RATE)).collect();
            out = vec![Vec::new(); input.len()];
        }
        for ((r, o), samples) in resamplers.iter_mut().zip(out.iter_mut()).zip(&input) {
            r.process(samples, o);
        }

        if let Some(total) = total_frames.filter(|t| *t > 0) {
            let p = (frames_done as f32 / total as f32).min(1.0);
            if p - last_report >= 0.01 {
                last_report = p;
                progress(p);
            }
        }
    }

    for (r, o) in resamplers.iter_mut().zip(out.iter_mut()) {
        r.flush(o);
    }
    if out.is_empty() || out[0].is_empty() {
        return Err(AudioError::Empty);
    }
    Ok(Raw { channels: out })
}

/// ffmpeg next to our own executable (a bundled sidecar), then the usual install locations.
/// GUI apps on macOS don't inherit the shell PATH, so Homebrew's dirs are listed explicitly.
pub fn find_ffmpeg() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(me) = std::env::current_exe() {
        if let Some(dir) = me.parent() {
            candidates.push(dir.join(exe));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(exe)));
    }
    if cfg!(target_os = "macos") {
        candidates.push("/opt/homebrew/bin/ffmpeg".into());
        candidates.push("/usr/local/bin/ffmpeg".into());
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn decode_ffmpeg(ffmpeg: &Path, path: &Path, cancel: &AtomicBool) -> Result<Raw, AudioError> {
    // Always ask for two channels: a mono source comes out as identical L/R, which
    // `finalize_channels` folds back to mono.
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-nostdin", "-v", "error", "-i"])
        .arg(path)
        .args(["-vn", "-f", "f32le", "-acodec", "pcm_f32le", "-ac", "2", "-ar", "16000", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| AudioError::Unsupported(e.to_string()))?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut carry: Vec<u8> = Vec::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            return Err(AudioError::Cancelled);
        }
        let n = stdout.read(&mut buf).map_err(|e| AudioError::Unsupported(e.to_string()))?;
        if n == 0 {
            break;
        }
        carry.extend_from_slice(&buf[..n]);
        let whole = carry.len() / 8 * 8;
        for frame in carry[..whole].chunks_exact(8) {
            left.push(f32::from_le_bytes(frame[0..4].try_into().unwrap()));
            right.push(f32::from_le_bytes(frame[4..8].try_into().unwrap()));
        }
        carry.drain(..whole);
    }
    let output = child.wait_with_output().map_err(|e| AudioError::Unsupported(e.to_string()))?;
    if left.is_empty() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if msg.is_empty() { AudioError::Empty } else { AudioError::Unsupported(msg) });
    }
    Ok(Raw { channels: vec![left, right] })
}

/// How the two sides of a stereo file relate to each other.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StereoKind {
    /// Two separate speakers on left and right: transcribe them one by one.
    Split,
    /// Same signal on both sides, or one side is silent: treat as mono.
    Mono,
}

/// Decide whether a stereo recording really has separate sides.
///
/// Many recorders write mono as stereo with identical channels; transcribing those
/// twice would duplicate every line. A channel that is (almost) silent is also mono.
pub fn classify_stereo(left: &[f32], right: &[f32]) -> StereoKind {
    let n = left.len().min(right.len());
    if n == 0 {
        return StereoKind::Mono;
    }
    let (mut el, mut er, mut ed) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (l, r) = (left[i] as f64, right[i] as f64);
        el += l * l;
        er += r * r;
        ed += (l - r) * (l - r);
    }
    let louder = el.max(er);
    if louder == 0.0 {
        return StereoKind::Mono;
    }
    // One side carries less than 1 % of the other's energy: nothing to transcribe there.
    if el.min(er) < louder * 0.01 {
        return StereoKind::Mono;
    }
    // The difference signal is tiny compared to the signal: the sides are copies.
    if ed < louder * 0.02 {
        return StereoKind::Mono;
    }
    StereoKind::Split
}

pub fn downmix(left: &[f32], right: &[f32]) -> Vec<f32> {
    left.iter().zip(right).map(|(l, r)| (l + r) * 0.5).collect()
}

fn finalize_channels(raw: Raw) -> Decoded {
    let source_channels = raw.channels.len();
    let mut channels = raw.channels;
    if channels.len() == 2 {
        let len = channels[0].len().min(channels[1].len());
        channels[0].truncate(len);
        channels[1].truncate(len);
        if classify_stereo(&channels[0], &channels[1]) == StereoKind::Mono {
            let right = channels.pop().unwrap();
            let mono = downmix(&channels[0], &right);
            channels[0] = mono;
        }
    }
    Decoded { channels, source_channels }
}

/// Streaming windowed-sinc resampler for one channel with a fixed ratio.
///
/// The filter kernel is tabulated once at high resolution, so each output sample is a
/// short dot product with linear interpolation into the table instead of `sin()` calls.
pub struct Resampler {
    step: f64,
    passthrough: bool,
    half_width: usize,
    table: Vec<f32>,
    table_res: usize,
    /// Input samples not yet consumed. `buf[0]` is input index `buf_start`.
    buf: Vec<f32>,
    buf_start: u64,
    /// Next output sample index.
    next_out: u64,
}

impl Resampler {
    const TABLE_RES: usize = 512;
    /// Zero crossings on each side of the kernel, measured at the output rate.
    const ZERO_CROSSINGS: usize = 16;

    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        let step = in_rate as f64 / out_rate as f64;
        // Low-pass at a bit under the lower Nyquist frequency, in units of input samples.
        let cutoff = (out_rate as f64 / in_rate as f64).min(1.0) * 0.95;
        let half_width = ((Self::ZERO_CROSSINGS as f64) / cutoff).ceil() as usize;
        let table_len = half_width * Self::TABLE_RES + 2;
        let table = (0..table_len)
            .map(|i| {
                let x = i as f64 / Self::TABLE_RES as f64;
                if x > half_width as f64 {
                    return 0.0;
                }
                let sinc = if x == 0.0 {
                    1.0
                } else {
                    let a = std::f64::consts::PI * x * cutoff;
                    a.sin() / a
                };
                // Blackman window over [-half_width, half_width].
                let w = {
                    let t = 0.5 + 0.5 * x / half_width as f64;
                    let tau = 2.0 * std::f64::consts::PI * t;
                    0.42 - 0.5 * tau.cos() + 0.08 * (2.0 * tau).cos()
                };
                (cutoff * sinc * w) as f32
            })
            .collect();
        Self {
            step,
            passthrough: in_rate == out_rate,
            half_width,
            table,
            table_res: Self::TABLE_RES,
            buf: Vec::new(),
            buf_start: 0,
            next_out: 0,
        }
    }

    #[inline]
    fn kernel(&self, dist: f64) -> f32 {
        let pos = dist.abs() * self.table_res as f64;
        let i = pos as usize;
        if i + 1 >= self.table.len() {
            return 0.0;
        }
        let frac = (pos - i as f64) as f32;
        self.table[i] * (1.0 - frac) + self.table[i + 1] * frac
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.passthrough {
            out.extend_from_slice(input);
            return;
        }
        self.buf.extend_from_slice(input);
        self.run(out, false);
    }

    pub fn flush(&mut self, out: &mut Vec<f32>) {
        if self.passthrough {
            return;
        }
        self.run(out, true);
        self.buf.clear();
    }

    fn run(&mut self, out: &mut Vec<f32>, flushing: bool) {
        let hw = self.half_width as i64;
        let buf_end = self.buf_start + self.buf.len() as u64;
        loop {
            let center = self.next_out as f64 * self.step;
            let c = center.floor() as i64;
            if flushing {
                if center >= buf_end as f64 {
                    break;
                }
            } else if c + hw >= buf_end as i64 {
                break;
            }
            let mut acc = 0.0f32;
            for j in (c - hw + 1)..=(c + hw) {
                if j < self.buf_start as i64 || j >= buf_end as i64 {
                    continue;
                }
                let s = self.buf[(j - self.buf_start as i64) as usize];
                acc += s * self.kernel(center - j as f64);
            }
            out.push(acc);
            self.next_out += 1;
        }
        // Drop input that no future output sample can reach.
        let center = self.next_out as f64 * self.step;
        let keep_from = (center.floor() as i64 - hw).max(self.buf_start as i64) as u64;
        let drop = (keep_from - self.buf_start) as usize;
        if drop > 0 && drop <= self.buf.len() {
            self.buf.drain(..drop);
            self.buf_start = keep_from;
        }
    }
}

/// Convenience for tests and small inputs: resample a whole buffer at once.
pub fn resample(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
    let mut r = Resampler::new(in_rate, out_rate);
    let mut out = Vec::with_capacity((input.len() as f64 * out_rate as f64 / in_rate as f64) as usize + 1);
    // Feed in uneven chunks so the streaming path is what gets exercised.
    for chunk in input.chunks(4_093) {
        r.process(chunk, &mut out);
    }
    r.flush(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn sine(freq: f32, rate: u32, secs: f32, amp: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| amp * (2.0 * PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// Correlation of `x` with a sine of `freq` at `rate`: ~amplitude/√2 if present.
    fn tone_level(x: &[f32], freq: f32, rate: u32) -> f32 {
        let (mut s, mut c) = (0.0f64, 0.0f64);
        for (i, v) in x.iter().enumerate() {
            let ph = 2.0 * std::f64::consts::PI * freq as f64 * i as f64 / rate as f64;
            s += *v as f64 * ph.sin();
            c += *v as f64 * ph.cos();
        }
        (((s * s + c * c).sqrt()) * 2.0 / x.len() as f64) as f32
    }

    #[test]
    fn resample_lengths_match_ratio() {
        for rate in [8_000u32, 11_025, 22_050, 32_000, 44_100, 48_000] {
            let input = vec![0.1f32; rate as usize * 3];
            let out = resample(&input, rate, 16_000);
            let expected = 48_000usize;
            assert!(
                (out.len() as i64 - expected as i64).abs() <= 2,
                "{rate} Hz: {} samples, expected ~{expected}",
                out.len()
            );
        }
    }

    #[test]
    fn resample_keeps_inband_tone() {
        for rate in [44_100u32, 48_000, 8_000] {
            let input = sine(440.0, rate, 2.0, 0.5);
            let out = resample(&input, rate, 16_000);
            // Skip the filter's edges.
            let mid = &out[2_000..out.len() - 2_000];
            let level = tone_level(mid, 440.0, 16_000);
            assert!((level - 0.5).abs() < 0.02, "{rate} Hz: tone level {level}");
        }
    }

    #[test]
    fn resample_removes_tone_above_nyquist() {
        // 12 kHz can't exist at 16 kHz; without filtering it would alias to 4 kHz.
        let input = sine(12_000.0, 48_000, 1.0, 0.5);
        let out = resample(&input, 48_000, 16_000);
        let mid = &out[1_000..out.len() - 1_000];
        assert!(rms(mid) < 0.01, "aliasing rms {}", rms(mid));
    }

    #[test]
    fn resample_passthrough_at_16k() {
        let input = sine(300.0, 16_000, 0.5, 0.3);
        assert_eq!(resample(&input, 16_000, 16_000), input);
    }

    #[test]
    fn stereo_identical_channels_is_mono() {
        let l = sine(200.0, 16_000, 1.0, 0.4);
        assert_eq!(classify_stereo(&l, &l), StereoKind::Mono);
    }

    #[test]
    fn stereo_silent_side_is_mono() {
        let l = sine(200.0, 16_000, 1.0, 0.4);
        let r = vec![0.0001f32; l.len()];
        assert_eq!(classify_stereo(&l, &r), StereoKind::Mono);
    }

    #[test]
    fn stereo_different_speakers_is_split() {
        // Speaker A talks in the first half on the left, speaker B in the second half on the right.
        let a = sine(220.0, 16_000, 1.0, 0.4);
        let b = sine(330.0, 16_000, 1.0, 0.4);
        let silence = vec![0.0f32; a.len()];
        let left = [a.clone(), silence.clone()].concat();
        let right = [silence, b].concat();
        assert_eq!(classify_stereo(&left, &right), StereoKind::Split);
    }

    #[test]
    fn wav_decodes_to_16k_mono() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for s in sine(440.0, 44_100, 2.0, 0.5) {
            w.write_sample((s * i16::MAX as f32) as i16).unwrap();
        }
        w.finalize().unwrap();

        let decoded = decode_file(&path, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(decoded.channels.len(), 1);
        assert_eq!(decoded.source_channels, 1);
        assert!((decoded.channels[0].len() as i64 - 32_000).abs() <= 2);
        assert!((decoded.duration_ms() as i64 - 2_000).abs() <= 1);
        let level = tone_level(&decoded.channels[0][2_000..30_000], 440.0, 16_000);
        assert!((level - 0.5).abs() < 0.02, "level {level}");
    }

    #[test]
    fn stereo_wav_with_two_sides_keeps_both_channels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("two-sides.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let a = sine(220.0, 48_000, 1.0, 0.4);
        let b = sine(330.0, 48_000, 1.0, 0.4);
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        // First second: left only. Second second: right only.
        for s in &a {
            w.write_sample((s * 32767.0) as i16).unwrap();
            w.write_sample(0i16).unwrap();
        }
        for s in &b {
            w.write_sample(0i16).unwrap();
            w.write_sample((s * 32767.0) as i16).unwrap();
        }
        w.finalize().unwrap();

        let decoded = decode_file(&path, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(decoded.source_channels, 2);
        assert_eq!(decoded.channels.len(), 2);
        let (l, r) = (&decoded.channels[0], &decoded.channels[1]);
        assert!(rms(&l[1_000..15_000]) > 0.2 && rms(&l[17_000..31_000]) < 0.01);
        assert!(rms(&r[1_000..15_000]) < 0.01 && rms(&r[17_000..31_000]) > 0.2);
    }

    #[test]
    fn dual_mono_wav_becomes_one_channel() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dual-mono.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for s in sine(300.0, 16_000, 1.0, 0.3) {
            let v = (s * 32767.0) as i16;
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
        let decoded = decode_file(&path, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(decoded.source_channels, 2);
        assert_eq!(decoded.channels.len(), 1);
    }

    #[test]
    fn missing_file_is_an_open_error() {
        let err = decode_file(Path::new("/finns/inte.wav"), &AtomicBool::new(false), |_| {});
        assert!(matches!(err, Err(AudioError::Open(_))));
    }
}
