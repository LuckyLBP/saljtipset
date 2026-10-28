//! Selling points from Claude: the prompt, the response schema, and validation.
//!
//! Validation has two layers. `parse_and_validate` rejects responses that are not the
//! JSON we asked for (the caller retries once). `verify_against_transcript` then drops
//! every point whose quote can't be found in the transcript, so nothing invented reaches
//! the user, and replaces the model's timestamp with the real one of the matching line.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::people::Profile;
use crate::transcript::{Segment, format_for_model, format_timestamp, parse_timestamp};

pub const ROLE_SELLER: &str = "Säljare";
pub const ROLE_CUSTOMER: &str = "Kund";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub quote: String,
    pub timestamp: String,
    /// Filled in by verification: the transcript line the quote starts in.
    #[serde(default)]
    pub segment_id: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SellingPoint {
    pub title: String,
    pub explanation: String,
    #[serde(flatten)]
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomerNeed {
    pub need: String,
    #[serde(flatten)]
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Objection {
    pub objection: String,
    pub suggested_answer: String,
    #[serde(flatten)]
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MissedOpportunity {
    pub description: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SegmentRole {
    pub segment_id: usize,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRole {
    pub speaker: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tips {
    /// Mono calls: Claude's guess of who says each line.
    pub speakers: Vec<SegmentRole>,
    /// Stereo calls: which channel is the seller and which is the customer.
    #[serde(default)]
    pub channel_roles: Vec<ChannelRole>,
    pub summary: String,
    pub selling_points: Vec<SellingPoint>,
    pub customer_needs: Vec<CustomerNeed>,
    pub objections: Vec<Objection>,
    pub missed_opportunities: Vec<MissedOpportunity>,
    pub next_steps: Vec<String>,
    /// The customer's name and company, only if said in the call (verified), else empty.
    #[serde(default)]
    pub customer_name: String,
    #[serde(default)]
    pub customer_company: String,
    /// How many points verification removed because their quote wasn't in the transcript.
    #[serde(default)]
    pub dropped_points: usize,
}

impl Tips {
    /// Role per segment id, for labelling a mono transcript.
    pub fn roles_by_segment(&self, n_segments: usize) -> Vec<Option<String>> {
        let mut roles = vec![None; n_segments];
        for r in &self.speakers {
            if let Some(slot) = roles.get_mut(r.segment_id) {
                *slot = Some(r.role.clone());
            }
        }
        roles
    }
}

pub const SYSTEM_PROMPT: &str = "\
Du är en erfaren svensk säljcoach. Du får en transkribering av ett säljsamtal och ska hjälpa \
säljaren att förstå vad som fungerade, vad kunden behöver och vad som kan göras bättre.

Regler:
- Svara alltid på svenska.
- Svara enbart med JSON enligt det angivna schemat, utan någon annan text.
- Varje citat (`quote`) ska vara en exakt, ordagrann del av transkriberingen: kopiera orden \
precis som de står, utan att rätta stavning eller grammatik, utan \"...\" och utan att foga ihop \
text från olika rader. Håll citaten korta (ett par meningar som mest).
- `timestamp` ska vara tidsstämpeln (mm:ss) för raden där citatet står.
- Hitta aldrig på citat, siffror eller händelser som inte finns i transkriberingen. Hellre färre \
punkter än påhittade. Tomma listor är okej.
- Transkriberingen är automatisk och kan innehålla felhörningar; tolka rimligt men citera exakt.";

/// The user message: instructions for this call plus the transcript.
pub fn user_prompt(segments: &[Segment], stereo: bool, profile: &Profile) -> String {
    let speakers = if stereo {
        "Samtalet är inspelat i stereo: \"Talare 1\" och \"Talare 2\" är säkra talare (olika kanaler). \
Avgör vilken som är säljaren och vilken som är kunden och fyll i `channelRoles` med en rad per \
talare (roll \"Säljare\" eller \"Kund\"). Lämna `speakers` tom."
    } else {
        "Samtalet är inspelat i mono och talarna är inte märkta. Gissa utifrån innehållet vem som säger \
vad och fyll i `speakers` med en rad per transkriberingsrad (`segmentId` är numret efter #, roll \
\"Säljare\" eller \"Kund\"). Lämna `channelRoles` tom."
    };
    let about = profile.prompt_block().map(|p| format!("{p}\n\n")).unwrap_or_default();
    format!(
        "Analysera säljsamtalet nedan.\n\n{about}{speakers}\n\nFyll i:\n\
- `customerName`: kundens namn om det sägs i samtalet (t.ex. när kunden presenterar sig eller blir \
tilltalad), annars tom sträng. Gissa inte.\n\
- `customerCompany`: kundens företag om det nämns, annars tom sträng.\n\
- `summary`: 3–5 meningar om samtalet.\n\
- `sellingPoints`: det som fungerade bra eller som kunden reagerade positivt på, med citat som belägg.\n\
- `customerNeeds`: behov och problem som kunden nämnde, med citat.\n\
- `objections`: kundens invändningar, med citat och ett förslag på hur säljaren kan bemöta dem.\n\
- `missedOpportunities`: vad säljaren hade kunnat säga eller fråga, och ett konkret förslag.\n\
- `nextSteps`: överenskomna eller rekommenderade nästa steg.\n\n\
Varje rad i transkriberingen har formatet `#id [mm:ss] Talare: text`.\n\n\
<transkribering>\n{}\n</transkribering>",
        format_for_model(segments)
    )
}

/// JSON schema for structured output. Mirrors [`Tips`] (minus the fields we fill in ourselves).
pub fn response_schema() -> serde_json::Value {
    let evidence_props = |extra: serde_json::Value| {
        let mut props = json!({
            "quote": {"type": "string"},
            "timestamp": {"type": "string"}
        });
        props.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        props
    };
    let role = json!({"type": "string", "enum": [ROLE_SELLER, ROLE_CUSTOMER]});
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["speakers", "channelRoles", "customerName", "customerCompany", "summary", "sellingPoints",
                     "customerNeeds", "objections", "missedOpportunities", "nextSteps"],
        "properties": {
            "customerName": {"type": "string"},
            "customerCompany": {"type": "string"},
            "speakers": {"type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["segmentId", "role"],
                "properties": {"segmentId": {"type": "integer"}, "role": role}
            }},
            "channelRoles": {"type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["speaker", "role"],
                "properties": {"speaker": {"type": "string", "enum": ["Talare 1", "Talare 2"]}, "role": role}
            }},
            "summary": {"type": "string"},
            "sellingPoints": {"type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["title", "explanation", "quote", "timestamp"],
                "properties": evidence_props(json!({"title": {"type": "string"}, "explanation": {"type": "string"}}))
            }},
            "customerNeeds": {"type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["need", "quote", "timestamp"],
                "properties": evidence_props(json!({"need": {"type": "string"}}))
            }},
            "objections": {"type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["objection", "suggestedAnswer", "quote", "timestamp"],
                "properties": evidence_props(json!({"objection": {"type": "string"}, "suggestedAnswer": {"type": "string"}}))
            }},
            "missedOpportunities": {"type": "array", "items": {
                "type": "object", "additionalProperties": false,
                "required": ["description", "suggestion"],
                "properties": {"description": {"type": "string"}, "suggestion": {"type": "string"}}
            }},
            "nextSteps": {"type": "array", "items": {"type": "string"}}
        }
    })
}

/// Parse the model's text into [`Tips`] and check the parts serde can't.
pub fn parse_and_validate(text: &str, n_segments: usize) -> Result<Tips, String> {
    let trimmed = strip_code_fence(text);
    let tips: Tips = serde_json::from_str(trimmed).map_err(|e| format!("ogiltig JSON: {e}"))?;
    let sentences = tips.summary.split(['.', '!', '?']).filter(|s| s.trim().len() > 1).count();
    if tips.summary.trim().is_empty() || sentences > 8 {
        return Err(format!("`summary` ska vara 3–5 meningar, fick {sentences}"));
    }
    for r in &tips.speakers {
        if r.segment_id >= n_segments {
            return Err(format!("`speakers` pekar på rad #{} som inte finns", r.segment_id));
        }
        if r.role != ROLE_SELLER && r.role != ROLE_CUSTOMER {
            return Err(format!("okänd roll \"{}\"", r.role));
        }
    }
    for r in &tips.channel_roles {
        if r.role != ROLE_SELLER && r.role != ROLE_CUSTOMER {
            return Err(format!("okänd roll \"{}\"", r.role));
        }
    }
    let all_evidence = tips
        .selling_points
        .iter()
        .map(|p| &p.evidence)
        .chain(tips.customer_needs.iter().map(|p| &p.evidence))
        .chain(tips.objections.iter().map(|p| &p.evidence));
    for e in all_evidence {
        if parse_timestamp(&e.timestamp).is_none() {
            return Err(format!("ogiltig tidsstämpel \"{}\"", e.timestamp));
        }
    }
    Ok(tips)
}

/// Models sometimes wrap JSON in ```json fences even when told not to.
fn strip_code_fence(text: &str) -> &str {
    let t = text.trim();
    let Some(rest) = t.strip_prefix("```") else { return t };
    let rest = rest.trim_start_matches("json").trim_start_matches("JSON");
    rest.strip_suffix("```").unwrap_or(rest).trim()
}

/// Lowercase, keep letters and digits, collapse everything else into single spaces.
/// Also returns, for each output char, the index of the source segment it came from.
fn normalize_with_map(segments: &[Segment]) -> (Vec<char>, Vec<usize>) {
    let mut chars = Vec::new();
    let mut owner = Vec::new();
    for s in segments {
        push_normalized(&s.text, s.id, &mut chars, &mut owner);
        if chars.last() != Some(&' ') {
            chars.push(' ');
            owner.push(s.id);
        }
    }
    (chars, owner)
}

fn push_normalized(text: &str, id: usize, chars: &mut Vec<char>, owner: &mut Vec<usize>) {
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            chars.push(c);
            owner.push(id);
        } else if !chars.is_empty() && chars.last() != Some(&' ') {
            chars.push(' ');
            owner.push(id);
        }
    }
}

fn normalize(text: &str) -> Vec<char> {
    let (mut chars, mut owner) = (Vec::new(), Vec::new());
    push_normalized(text, 0, &mut chars, &mut owner);
    while chars.last() == Some(&' ') {
        chars.pop();
    }
    chars
}

/// The segment a quote starts in, if the quote appears in the transcript (ignoring case,
/// punctuation and spacing, and allowing it to run across consecutive lines).
/// If it appears several times, the occurrence closest to `hint_ms` wins.
pub fn find_quote(segments: &[Segment], quote: &str, hint_ms: Option<u64>) -> Option<usize> {
    let needle = normalize(quote);
    // A couple of words at least; single short words match anywhere and prove nothing.
    if needle.iter().filter(|c| c.is_alphanumeric()).count() < 6 {
        return None;
    }
    let (hay, owner) = normalize_with_map(segments);
    let mut best: Option<(u64, usize)> = None;
    if needle.len() > hay.len() {
        return None;
    }
    for start in 0..=hay.len() - needle.len() {
        if hay[start..start + needle.len()] == needle[..] {
            // Only accept matches that begin at a word boundary.
            if start > 0 && hay[start - 1] != ' ' {
                continue;
            }
            let id = owner[start];
            let seg_start = segments.iter().find(|s| s.id == id).map_or(0, |s| s.start_ms);
            let dist = hint_ms.map_or(0, |h| seg_start.abs_diff(h));
            if best.is_none_or(|(d, _)| dist < d) {
                best = Some((dist, id));
            }
        }
    }
    best.map(|(_, id)| id)
}

/// Whether `name` occurs as whole words in the transcript (ignoring case and punctuation).
pub fn is_mentioned(segments: &[Segment], name: &str) -> bool {
    let needle = normalize(name);
    if needle.iter().filter(|c| c.is_alphanumeric()).count() < 2 {
        return false;
    }
    let (hay, _) = normalize_with_map(segments);
    let hay: String = std::iter::once(' ').chain(hay).chain(std::iter::once(' ')).collect();
    let needle: String = std::iter::once(' ').chain(needle).chain(std::iter::once(' ')).collect();
    hay.contains(&needle)
}

/// Drop points whose quote isn't in the transcript and correct the timestamps of the rest.
pub fn verify_against_transcript(mut tips: Tips, segments: &[Segment]) -> Tips {
    let mut dropped = 0;
    let mut check = |e: &mut Evidence| -> bool {
        match find_quote(segments, &e.quote, parse_timestamp(&e.timestamp)) {
            Some(id) => {
                let start = segments.iter().find(|s| s.id == id).map_or(0, |s| s.start_ms);
                e.timestamp = format_timestamp(start);
                e.segment_id = Some(id);
                e.quote = e.quote.trim().to_string();
                true
            }
            None => {
                dropped += 1;
                false
            }
        }
    };
    tips.selling_points.retain_mut(|p| check(&mut p.evidence));
    tips.customer_needs.retain_mut(|p| check(&mut p.evidence));
    tips.objections.retain_mut(|p| check(&mut p.evidence));
    tips.speakers.retain(|r| r.segment_id < segments.len());
    // Names are only shown if they were actually said.
    for field in [&mut tips.customer_name, &mut tips.customer_company] {
        if !is_mentioned(segments, field) {
            field.clear();
        }
        *field = field.trim().to_string();
    }
    tips.dropped_points = dropped;
    tips
}

/// "Kopiera säljtips": the tips as readable plain text.
pub fn format_tips_plain(tips: &Tips, title: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("Säljtips – {title}\n\n"));
    out.push_str("Sammanfattning\n");
    out.push_str(tips.summary.trim());
    out.push_str("\n\n");

    let section = |out: &mut String, heading: &str, items: Vec<String>| {
        if items.is_empty() {
            return;
        }
        out.push_str(heading);
        out.push('\n');
        for item in items {
            out.push_str(&item);
            out.push('\n');
        }
        out.push('\n');
    };
    section(
        &mut out,
        "Det här fungerade",
        tips.selling_points
            .iter()
            .map(|p| format!("• {}\n  {}\n  [{}] \"{}\"", p.title, p.explanation, p.evidence.timestamp, p.evidence.quote))
            .collect(),
    );
    section(
        &mut out,
        "Kundens behov",
        tips.customer_needs
            .iter()
            .map(|p| format!("• {}\n  [{}] \"{}\"", p.need, p.evidence.timestamp, p.evidence.quote))
            .collect(),
    );
    section(
        &mut out,
        "Invändningar",
        tips.objections
            .iter()
            .map(|p| {
                format!(
                    "• {}\n  [{}] \"{}\"\n  Förslag: {}",
                    p.objection, p.evidence.timestamp, p.evidence.quote, p.suggested_answer
                )
            })
            .collect(),
    );
    section(
        &mut out,
        "Missade möjligheter",
        tips.missed_opportunities.iter().map(|p| format!("• {}\n  Förslag: {}", p.description, p.suggestion)).collect(),
    );
    section(&mut out, "Nästa steg", tips.next_steps.iter().map(|s| format!("• {s}")).collect());
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments() -> Vec<Segment> {
        let lines = [
            (0, "Hej, det är Anna från Prospex. Har du två minuter?", "Talare 1"),
            (4_000, "Ja, det har jag. Vi har faktiskt problem med vår uppföljning av leads.", "Talare 2"),
            (11_000, "Vad bra att du säger det. Vårt system skickar påminnelser automatiskt,", "Talare 1"),
            (15_000, "så ingen kund faller mellan stolarna.", "Talare 1"),
            (19_000, "Det låter riktigt bra, men det är nog för dyrt för oss.", "Talare 2"),
            (65_000, "Det låter riktigt bra faktiskt.", "Talare 2"),
        ];
        lines
            .iter()
            .enumerate()
            .map(|(i, (ms, text, who))| Segment {
                id: i,
                start_ms: *ms,
                end_ms: ms + 3_000,
                text: text.to_string(),
                speaker: Some(who.to_string()),
            })
            .collect()
    }

    fn valid_json() -> String {
        json!({
            "speakers": [],
            "channelRoles": [{"speaker": "Talare 1", "role": "Säljare"}, {"speaker": "Talare 2", "role": "Kund"}],
            "summary": "Anna ringer en kund. Kunden har problem med uppföljning. Anna visar automatiska påminnelser.",
            "sellingPoints": [
                {"title": "Automatiska påminnelser", "explanation": "Kunden gillade det.",
                 "quote": "Det låter riktigt bra", "timestamp": "00:20"},
                {"title": "Påhittad", "explanation": "Finns inte.",
                 "quote": "Vi skriver på direkt i dag", "timestamp": "00:30"}
            ],
            "customerNeeds": [
                {"need": "Bättre uppföljning", "quote": "problem med vår uppföljning av leads", "timestamp": "00:04"}
            ],
            "objections": [
                {"objection": "Priset", "quote": "det är nog för dyrt för oss", "timestamp": "00:19",
                 "suggestedAnswer": "Räkna på vad tappade leads kostar."}
            ],
            "missedOpportunities": [{"description": "Frågade inte om budget.", "suggestion": "Fråga om budget tidigt."}],
            "nextSteps": ["Boka demo"],
            "customerName": "Anna",
            "customerCompany": "Volvo"
        })
        .to_string()
    }

    #[test]
    fn valid_response_parses() {
        let tips = parse_and_validate(&valid_json(), 6).unwrap();
        assert_eq!(tips.selling_points.len(), 2);
        assert_eq!(tips.channel_roles[0].role, "Säljare");
    }

    #[test]
    fn code_fenced_response_parses() {
        let fenced = format!("```json\n{}\n```", valid_json());
        assert!(parse_and_validate(&fenced, 6).is_ok());
    }

    #[test]
    fn broken_json_is_rejected() {
        let err = parse_and_validate("{\"summary\": \"hej\"", 6).unwrap_err();
        assert!(err.contains("ogiltig JSON"));
    }

    #[test]
    fn missing_field_is_rejected() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        v.as_object_mut().unwrap().remove("objections");
        assert!(parse_and_validate(&v.to_string(), 6).is_err());
    }

    #[test]
    fn bad_role_and_bad_segment_are_rejected() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        v["speakers"] = json!([{"segmentId": 99, "role": "Säljare"}]);
        assert!(parse_and_validate(&v.to_string(), 6).unwrap_err().contains("#99"));
        v["speakers"] = json!([{"segmentId": 1, "role": "Chef"}]);
        assert!(parse_and_validate(&v.to_string(), 6).unwrap_err().contains("Chef"));
    }

    #[test]
    fn bad_timestamp_is_rejected() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_json()).unwrap();
        v["customerNeeds"][0]["timestamp"] = json!("strax efter start");
        assert!(parse_and_validate(&v.to_string(), 6).unwrap_err().contains("tidsstämpel"));
    }

    #[test]
    fn invented_quotes_are_dropped_and_timestamps_corrected() {
        let tips = parse_and_validate(&valid_json(), 6).unwrap();
        let tips = verify_against_transcript(tips, &segments());
        assert_eq!(tips.selling_points.len(), 1, "the invented quote must go");
        assert_eq!(tips.dropped_points, 1);
        // "Det låter riktigt bra" appears at 00:19 and 01:05; the hint 00:20 picks the first.
        assert_eq!(tips.selling_points[0].evidence.segment_id, Some(4));
        assert_eq!(tips.selling_points[0].evidence.timestamp, "00:19");
        assert_eq!(tips.customer_needs[0].evidence.segment_id, Some(1));
        assert_eq!(tips.objections[0].evidence.segment_id, Some(4));
    }

    #[test]
    fn customer_names_are_kept_only_if_said() {
        let tips = verify_against_transcript(parse_and_validate(&valid_json(), 6).unwrap(), &segments());
        assert_eq!(tips.customer_name, "Anna", "said in segment 0");
        assert_eq!(tips.customer_company, "", "never said, so dropped");
        assert!(is_mentioned(&segments(), "prospex"));
        assert!(!is_mentioned(&segments(), "Ann"), "whole words only");
    }

    #[test]
    fn quote_matching_ignores_case_punctuation_and_line_breaks() {
        let segs = segments();
        assert_eq!(find_quote(&segs, "VAD BRA att du säger det!", None), Some(2));
        // Spans segments 2 and 3.
        assert_eq!(find_quote(&segs, "skickar påminnelser automatiskt så ingen kund faller", None), Some(2));
        // Repeated quote: the hint picks the later occurrence.
        assert_eq!(find_quote(&segs, "det låter riktigt bra", Some(64_000)), Some(5));
    }

    #[test]
    fn quote_matching_rejects_near_misses() {
        let segs = segments();
        assert_eq!(find_quote(&segs, "Det låter jättebra", None), None);
        assert_eq!(find_quote(&segs, "Hej", None), None, "too short to prove anything");
        assert_eq!(find_quote(&segs, "ej, det är Anna", None), None, "must start at a word boundary");
        assert_eq!(find_quote(&segs, "", None), None);
    }

    #[test]
    fn roles_by_segment_maps_ids() {
        let mut tips = parse_and_validate(&valid_json(), 6).unwrap();
        tips.speakers = vec![SegmentRole { segment_id: 1, role: "Kund".into() }];
        let roles = tips.roles_by_segment(3);
        assert_eq!(roles, vec![None, Some("Kund".into()), None]);
    }

    #[test]
    fn plain_text_has_all_sections() {
        let tips = verify_against_transcript(parse_and_validate(&valid_json(), 6).unwrap(), &segments());
        let text = format_tips_plain(&tips, "Samtal med Anna");
        assert!(text.starts_with("Säljtips – Samtal med Anna\n\nSammanfattning\n"));
        for heading in ["Det här fungerade", "Kundens behov", "Invändningar", "Missade möjligheter", "Nästa steg"] {
            assert!(text.contains(heading), "missing {heading}");
        }
        assert!(text.contains("[00:19] \"Det låter riktigt bra\""));
        assert!(text.contains("Förslag: Räkna på vad tappade leads kostar."));
    }

    #[test]
    fn schema_requires_every_field() {
        let schema = response_schema();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), schema["properties"].as_object().unwrap().len());
    }
}
