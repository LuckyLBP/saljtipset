//! Transcribe a file from the command line, for checking speed and memory on long calls.
//!
//!     cargo run --release --features gpu-metal --example transcribe -- <model.bin> <audio> [--cpu] [--threads N] [--all]

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bubbe_lib::transcribe::{self, ContextCache, Phase, Sink};
use bubbe_lib::transcript::{Segment, format_for_copy};

struct Progress(Mutex<(Instant, i32)>);

impl Sink for Progress {
    fn progress(&self, phase: Phase, percent: f32) {
        let mut last = self.0.lock().unwrap();
        if percent as i32 / 10 != last.1 / 10 {
            eprintln!("{:>6.1}s {:?} {:.0} %", last.0.elapsed().as_secs_f32(), phase, percent);
            last.1 = percent as i32;
        }
    }
    fn segment(&self, _: &Segment) {}
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (model, audio) = (&args[1], &args[2]);
    let use_gpu = !args.iter().any(|a| a == "--cpu");
    let mut opts = transcribe::Options { use_gpu, ..Default::default() };
    // --threads N: mimic a smaller machine.
    if let Some(n) = args.iter().position(|a| a == "--threads").and_then(|i| args.get(i + 1)) {
        opts.threads = n.parse().expect("--threads takes a number");
    }
    let started = Instant::now();
    let out = transcribe::transcribe_file(
        &ContextCache::default(),
        model.as_ref(),
        audio.as_ref(),
        &opts,
        Arc::new(AtomicBool::new(false)),
        Arc::new(Progress(Mutex::new((Instant::now(), -10)))),
    )
    .expect("transcription failed");
    let secs = started.elapsed().as_secs_f64();
    let text = format_for_copy(&out.segments, &[]);
    // --all: the whole transcript, e.g. to compare two runs.
    let shown = if args.iter().any(|a| a == "--all") { usize::MAX } else { 6 };
    println!("{}", text.lines().take(shown).collect::<Vec<_>>().join("\n"));
    println!(
        "…\n{} segments, {} channel(s), audio {:.1} min, took {:.1} s ({:.0}x realtime, gpu: {use_gpu}, threads: {})",
        out.segments.len(),
        out.channels,
        out.duration_ms as f64 / 60_000.0,
        secs,
        out.duration_ms as f64 / 1000.0 / secs,
        opts.threads
    );
}
