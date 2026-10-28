//! Energy-based speech detection: which parts of a channel to send to whisper.
//!
//! Whisper places timestamps poorly after long silences, and on a stereo call each side is
//! silent while the other party talks: a line said at 0:16 can come back as 0:10, which
//! breaks merging the two sides by time. Gluing the speech together doesn't help either,
//! because whisper then runs one sentence across two turns. So we find the turns and
//! transcribe each one on its own, offset by where it starts in the file. A channel with no
//! speech at all is skipped, which also stops whisper from hallucinating text into silence.

use crate::audio::TARGET_RATE;

const FRAME: usize = (TARGET_RATE as usize) / 50; // 20 ms
/// Pauses shorter than this stay inside one turn.
const TURN_GAP_MS: u64 = 2_000;
/// Context kept around each turn so word onsets aren't clipped.
const PAD_MS: u64 = 250;
/// Shorter blips (clicks, breaths) are ignored.
const MIN_SPEECH_MS: u64 = 200;

pub fn ms_to_samples(ms: u64) -> usize {
    (ms * TARGET_RATE as u64 / 1000) as usize
}

pub fn samples_to_ms(n: usize) -> u64 {
    n as u64 * 1000 / TARGET_RATE as u64
}

/// Speech turns as sample ranges `[start, end)`, in order.
pub fn speech_turns(samples: &[f32]) -> Vec<(usize, usize)> {
    let n_frames = samples.len() / FRAME;
    if n_frames == 0 {
        return vec![];
    }
    let db: Vec<f32> = samples
        .chunks(FRAME)
        .take(n_frames)
        .map(|f| {
            let rms = (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt();
            20.0 * rms.max(1e-6).log10()
        })
        .collect();
    let mut sorted = db.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let floor = sorted[sorted.len() / 10];
    let loud = sorted[sorted.len() * 98 / 100];
    // Nothing stands out from the floor: silence or constant noise.
    if loud - floor < 15.0 || loud < -50.0 {
        return vec![];
    }
    // A third of the way (in dB) from the noise floor to loud speech, never below -50 dBFS.
    let threshold = (floor + (loud - floor) * 0.35).max(-50.0);

    let mut regions: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for (i, d) in db.iter().enumerate() {
        match (start, *d >= threshold) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                regions.push((s * FRAME, i * FRAME));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        regions.push((s * FRAME, n_frames * FRAME));
    }

    let min_len = ms_to_samples(MIN_SPEECH_MS);
    let turn_gap = ms_to_samples(TURN_GAP_MS);
    let mut turns: Vec<(usize, usize)> = Vec::new();
    for r in regions {
        match turns.last_mut() {
            Some(last) if r.0 - last.1 <= turn_gap => last.1 = r.1,
            _ => turns.push(r),
        }
    }
    let pad = ms_to_samples(PAD_MS);
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (s, e) in turns.into_iter().filter(|(s, e)| e - s >= min_len) {
        let (s, e) = (s.saturating_sub(pad), (e + pad).min(samples.len()));
        match out.last_mut() {
            Some(last) if s <= last.1 => last.1 = e,
            _ => out.push((s, e)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: u64) -> Vec<f32> {
        (0..ms_to_samples(ms)).map(|i| 0.3 * (i as f32 * 0.2).sin()).collect()
    }

    fn silence(ms: u64) -> Vec<f32> {
        // A little noise, like a real line.
        (0..ms_to_samples(ms)).map(|i| 0.0005 * ((i * 7919 % 101) as f32 / 50.0 - 1.0)).collect()
    }

    fn starts_ms(turns: &[(usize, usize)]) -> Vec<u64> {
        turns.iter().map(|t| samples_to_ms(t.0)).collect()
    }

    #[test]
    fn finds_turns_after_long_silence() {
        let audio = [silence(4_500), tone(2_000), silence(6_000), tone(1_000)].concat();
        let turns = speech_turns(&audio);
        assert_eq!(turns.len(), 2, "{turns:?}");
        let starts = starts_ms(&turns);
        assert!((starts[0] as i64 - 4_250).abs() <= 40, "{starts:?}");
        assert!((starts[1] as i64 - 12_250).abs() <= 40, "{starts:?}");
    }

    #[test]
    fn short_pauses_stay_in_one_turn() {
        let audio = [silence(1_000), tone(800), silence(1_500), tone(800), silence(1_000)].concat();
        assert_eq!(speech_turns(&audio).len(), 1);
    }

    #[test]
    fn blips_are_ignored() {
        let audio = [silence(3_000), tone(100), silence(3_000), tone(1_000), silence(500)].concat();
        let turns = speech_turns(&audio);
        assert_eq!(turns.len(), 1);
        assert!((starts_ms(&turns)[0] as i64 - 5_850).abs() <= 40);
    }

    #[test]
    fn silence_only_has_no_speech() {
        assert!(speech_turns(&silence(5_000)).is_empty());
        assert!(speech_turns(&vec![0.0; 16_000]).is_empty());
        assert!(speech_turns(&[]).is_empty());
    }
}
