//! The transcript data model and its plain-text formats.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    /// Position in the merged transcript. Stable once the transcription is finished.
    pub id: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    /// "Talare 1" / "Talare 2" for stereo calls, `None` for mono.
    pub speaker: Option<String>,
}

pub fn speaker_for_channel(channel: usize, n_channels: usize) -> Option<String> {
    (n_channels > 1).then(|| format!("Talare {}", channel + 1))
}

/// `mm:ss`, with minutes running past 59 for long calls (`75:12`).
pub fn format_timestamp(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

/// Parse `mm:ss` or `h:mm:ss` back to milliseconds.
pub fn parse_timestamp(ts: &str) -> Option<u64> {
    let parts: Vec<u64> = ts.trim().trim_matches(['[', ']']).split(':').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let secs = match parts.as_slice() {
        [m, s] if *s < 60 => m * 60 + s,
        [h, m, s] if *m < 60 && *s < 60 => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(secs * 1000)
}

/// Merge per-channel segments into one transcript ordered by start time and renumber them.
/// Ties keep channel order, so "Talare 1" comes first when both start at once.
pub fn merge_channels(per_channel: Vec<Vec<Segment>>) -> Vec<Segment> {
    let mut all: Vec<(usize, Segment)> = per_channel
        .into_iter()
        .enumerate()
        .flat_map(|(ch, segs)| segs.into_iter().map(move |s| (ch, s)))
        .collect();
    all.sort_by(|(ca, a), (cb, b)| a.start_ms.cmp(&b.start_ms).then(ca.cmp(cb)).then(a.end_ms.cmp(&b.end_ms)));
    all.into_iter()
        .enumerate()
        .map(|(i, (_, mut s))| {
            s.id = i;
            s
        })
        .collect()
}

/// Whisper output cleanup: trim, drop empty lines and the bracketed non-speech markers
/// whisper sometimes emits for silence (`[MUSIK]`, `(skratt)`).
pub fn clean_text(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let is_marker = (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')'));
    if is_marker && !t[1..t.len() - 1].contains(['[', ']', '(', ')']) {
        return None;
    }
    Some(t.to_string())
}

/// The label shown for a segment: a name or role when known (see people.rs), else the channel.
pub fn label_for<'a>(seg: &'a Segment, labels: &'a [Option<String>]) -> Option<&'a str> {
    labels.get(seg.id).and_then(|r| r.as_deref()).or(seg.speaker.as_deref())
}

/// The "Kopiera transkribering" format: one `[mm:ss] Talare: text` line per segment.
/// Segments without any speaker label are written as `[mm:ss] text`.
pub fn format_for_copy(segments: &[Segment], roles: &[Option<String>]) -> String {
    segments
        .iter()
        .map(|s| match label_for(s, roles) {
            Some(label) => format!("[{}] {}: {}", format_timestamp(s.start_ms), label, s.text),
            None => format!("[{}] {}", format_timestamp(s.start_ms), s.text),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The transcript as sent to Claude: like the copy format but with the segment id in front,
/// so the model can point back at exact lines.
pub fn format_for_model(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| match &s.speaker {
            Some(label) => format!("#{} [{}] {}: {}", s.id, format_timestamp(s.start_ms), label, s.text),
            None => format!("#{} [{}] {}", s.id, format_timestamp(s.start_ms), s.text),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(id: usize, start_ms: u64, text: &str, speaker: Option<&str>) -> Segment {
        Segment { id, start_ms, end_ms: start_ms + 1500, text: text.into(), speaker: speaker.map(Into::into) }
    }

    #[test]
    fn timestamps_format_as_minutes_and_seconds() {
        assert_eq!(format_timestamp(0), "00:00");
        assert_eq!(format_timestamp(65_400), "01:05");
        assert_eq!(format_timestamp(4_512_000), "75:12");
    }

    #[test]
    fn timestamps_parse_back() {
        assert_eq!(parse_timestamp("01:05"), Some(65_000));
        assert_eq!(parse_timestamp("[75:12]"), Some(4_512_000));
        assert_eq!(parse_timestamp("1:02:03"), Some(3_723_000));
        assert_eq!(parse_timestamp("1:75"), None);
        assert_eq!(parse_timestamp("abc"), None);
    }

    #[test]
    fn copy_format_with_speakers() {
        let segs = vec![
            seg(0, 1_000, "Hej, det är Anna från Prospex.", Some("Talare 1")),
            seg(1, 4_200, "Hej Anna!", Some("Talare 2")),
        ];
        assert_eq!(
            format_for_copy(&segs, &[]),
            "[00:01] Talare 1: Hej, det är Anna från Prospex.\n[00:04] Talare 2: Hej Anna!"
        );
    }

    #[test]
    fn copy_format_mono_uses_guessed_roles_when_present() {
        let segs = vec![seg(0, 0, "Hej!", None), seg(1, 62_000, "Vad kostar det?", None)];
        assert_eq!(format_for_copy(&segs, &[]), "[00:00] Hej!\n[01:02] Vad kostar det?");
        let roles = vec![Some("Säljare".to_string()), Some("Kund".to_string())];
        assert_eq!(format_for_copy(&segs, &roles), "[00:00] Säljare: Hej!\n[01:02] Kund: Vad kostar det?");
    }

    #[test]
    fn model_format_has_segment_ids() {
        let segs = vec![seg(0, 0, "Hej!", Some("Talare 1")), seg(1, 3_000, "Tja.", Some("Talare 2"))];
        assert_eq!(format_for_model(&segs), "#0 [00:00] Talare 1: Hej!\n#1 [00:03] Talare 2: Tja.");
    }

    #[test]
    fn merge_interleaves_channels_by_time() {
        let left = vec![seg(0, 0, "A1", Some("Talare 1")), seg(1, 10_000, "A2", Some("Talare 1"))];
        let right = vec![seg(0, 4_000, "B1", Some("Talare 2")), seg(1, 10_000, "B2", Some("Talare 2"))];
        let merged = merge_channels(vec![left, right]);
        let order: Vec<_> = merged.iter().map(|s| (s.id, s.text.as_str())).collect();
        assert_eq!(order, vec![(0, "A1"), (1, "B1"), (2, "A2"), (3, "B2")]);
    }

    #[test]
    fn clean_text_drops_markers_and_blanks() {
        assert_eq!(clean_text("  Hej där  "), Some("Hej där".into()));
        assert_eq!(clean_text("[MUSIK]"), None);
        assert_eq!(clean_text("(skratt)"), None);
        assert_eq!(clean_text("   "), None);
        assert_eq!(clean_text("(ja) det stämmer (nej)"), Some("(ja) det stämmer (nej)".into()));
    }
}
