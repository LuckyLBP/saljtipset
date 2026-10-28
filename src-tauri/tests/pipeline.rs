//! End-to-end tests on the Swedish fixtures in tests/fixtures.
//!
//! Decoding tests always run. The transcription tests need a KB-Whisper model and run when
//! `SALJTIPSET_TEST_MODEL` points at one (CI downloads `kb-whisper-small` q5_0):
//!
//!     SALJTIPSET_TEST_MODEL=/path/to/ggml-model-q5_0.bin cargo test --test pipeline

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use bubbe_lib::audio;
use bubbe_lib::transcribe::{self, ContextCache, Phase, Sink};
use bubbe_lib::transcript::{Segment, format_for_copy};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn decode(name: &str) -> audio::Decoded {
    audio::decode_file(&fixture(name), &AtomicBool::new(false), |_| {}).unwrap()
}

#[test]
fn decodes_wav_mp3_and_m4a_to_the_same_length() {
    for name in ["sv_mono.wav", "sv_mono.mp3", "sv_mono.m4a"] {
        let d = decode(name);
        assert_eq!(d.channels.len(), 1, "{name}");
        let ms = d.duration_ms() as i64;
        // Encoders add a little padding; allow 100 ms.
        assert!((ms - 9_017).abs() < 100, "{name}: {ms} ms");
    }
}

#[test]
fn decodes_webm_opus_when_ffmpeg_is_available() {
    if audio::find_ffmpeg().is_none() {
        eprintln!("skipping: no ffmpeg on this machine");
        return;
    }
    let d = decode("sv_mono.webm");
    assert_eq!(d.channels.len(), 1, "mono source must not become two speakers");
    assert!((d.duration_ms() as i64 - 9_025).abs() < 150);
}

#[test]
fn stereo_fixture_keeps_two_sides() {
    let d = decode("sv_stereo.wav");
    assert_eq!(d.source_channels, 2);
    assert_eq!(d.channels.len(), 2);
    assert_eq!(d.duration_ms(), 21_000);
}

struct Collect {
    segments: Mutex<Vec<Segment>>,
    last_progress: Mutex<f32>,
}

impl Sink for Collect {
    fn progress(&self, phase: Phase, percent: f32) {
        if matches!(phase, Phase::Transcribing) {
            *self.last_progress.lock().unwrap() = percent;
        }
    }
    fn segment(&self, segment: &Segment) {
        self.segments.lock().unwrap().push(segment.clone());
    }
}

fn model() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("SALJTIPSET_TEST_MODEL")?);
    assert!(p.is_file(), "SALJTIPSET_TEST_MODEL does not exist: {}", p.display());
    Some(p)
}

fn run(model: &Path, name: &str) -> (transcribe::Output, Arc<Collect>) {
    let sink = Arc::new(Collect { segments: Mutex::new(vec![]), last_progress: Mutex::new(0.0) });
    // CI runners have no usable GPU; locally this also keeps the test deterministic.
    let opts = transcribe::Options { use_gpu: false, ..Default::default() };
    let out = transcribe::transcribe_file(
        &ContextCache::default(),
        model,
        &fixture(name),
        &opts,
        Arc::new(AtomicBool::new(false)),
        sink.clone(),
    )
    .unwrap();
    (out, sink)
}

fn words(segments: &[Segment]) -> String {
    segments.iter().map(|s| s.text.to_lowercase()).collect::<Vec<_>>().join(" ")
}

#[test]
fn transcribes_swedish_mono() {
    let Some(model) = model() else {
        eprintln!("skipping: SALJTIPSET_TEST_MODEL not set");
        return;
    };
    let (out, sink) = run(&model, "sv_mono.wav");
    let text = words(&out.segments);
    println!("{}", format_for_copy(&out.segments, &[]));
    // Brand names vary ("Prospex" comes out as "Prospects"); check ordinary words.
    for w in ["anna", "uppföljning", "två minuter"] {
        assert!(text.contains(w), "expected \"{w}\" in: {text}");
    }
    assert!(out.segments.iter().all(|s| s.speaker.is_none()));
    assert!(!sink.segments.lock().unwrap().is_empty(), "segments must stream while transcribing");
}

#[test]
fn transcribes_stereo_as_two_speakers_in_time_order() {
    let Some(model) = model() else {
        eprintln!("skipping: SALJTIPSET_TEST_MODEL not set");
        return;
    };
    let (out, _) = run(&model, "sv_stereo.wav");
    println!("{}", format_for_copy(&out.segments, &[]));
    assert_eq!(out.channels, 2);
    let starts: Vec<u64> = out.segments.iter().map(|s| s.start_ms).collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]), "not sorted: {starts:?}");
    let speakers: Vec<&str> = out.segments.iter().map(|s| s.speaker.as_deref().unwrap()).collect();
    // Seller (left) opens, customer (right) answers, and so on.
    assert_eq!(speakers.first(), Some(&"Talare 1"));
    assert!(speakers.contains(&"Talare 2"));
    let t1 = words(&out.segments.iter().filter(|s| s.speaker.as_deref() == Some("Talare 1")).cloned().collect::<Vec<_>>());
    let t2 = words(&out.segments.iter().filter(|s| s.speaker.as_deref() == Some("Talare 2")).cloned().collect::<Vec<_>>());
    assert!(t1.contains("påminnelser"), "seller side: {t1}");
    assert!(t2.contains("dyrt"), "customer side: {t2}");
    assert!(!t1.contains("dyrt"), "customer leaked into seller channel: {t1}");
    // Timestamps must match where each line really is in the file (see the fixture layout in
    // the README: seller at 0 s and 10.5 s, customer at 4.5 s and 16.5 s).
    let start_of = |needle: &str| {
        out.segments.iter().find(|s| s.text.to_lowercase().contains(needle)).map(|s| s.start_ms as i64).unwrap()
    };
    for (needle, expected) in [("absolut", 4_500), ("vad bra", 10_500), ("dyrt", 16_500)] {
        let got = start_of(needle);
        assert!((got - expected).abs() <= 1_000, "\"{needle}\" at {got} ms, expected ~{expected} ms");
    }
}
