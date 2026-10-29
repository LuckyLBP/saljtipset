//! Who's who: the seller's own profile (from onboarding), which line belongs to the seller or
//! the customer, the names shown in the transcript, and simple talk statistics.

use serde::{Deserialize, Serialize};

use crate::tips::{Kind, ROLE_CUSTOMER, ROLE_SELLER, Tips};

pub const ROLE_ADMIN: &str = "admin";
use crate::transcript::Segment;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    /// "kvinna", "man" or "annat" (or empty). Only used to pick a pronoun for the AI.
    pub gender: String,
    pub company: String,
    /// What the seller sells and their main arguments, in their own words.
    pub offer: String,
    /// "saljare" (coaching for your own calls) or [`ROLE_ADMIN`] (reviewing others' calls).
    /// Profiles from before roles existed are sellers.
    #[serde(default)]
    pub role: String,
    /// Admin: what to flag, one rule per line, in their own words.
    #[serde(default)]
    pub policy: String,
    /// Admin: the company's "Allmänna villkor" as text. Not part of the stored profile or
    /// sent to the UI: loaded from its own setting when the AI needs it (see terms.rs).
    #[serde(skip)]
    pub terms: String,
}

impl Profile {
    pub fn pronoun(&self) -> &'static str {
        match self.gender.as_str() {
            "kvinna" => "hon",
            "man" => "han",
            _ => "hen",
        }
    }

    pub fn first_name(&self) -> Option<&str> {
        self.name.split_whitespace().next()
    }

    pub fn is_admin(&self) -> bool {
        self.role == ROLE_ADMIN
    }

    /// The policy as numbered lines, so flags can point at "regel 3". Lines that already
    /// start with a number keep it.
    pub fn numbered_policy(&self) -> String {
        let lines = self.policy.lines().map(str::trim).filter(|l| !l.is_empty());
        let mut out = Vec::new();
        for (i, line) in lines.enumerate() {
            let line = line.trim_start_matches(['-', '•', '*']).trim();
            if line.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                out.push(line.to_string());
            } else {
                out.push(format!("{}. {line}", i + 1));
            }
        }
        out.join("\n")
    }

    /// Context for the AI: the person being coached, or for an admin, who is reviewing and
    /// against which rules.
    pub fn prompt_block(&self) -> Option<String> {
        if self.is_admin() {
            return self.admin_prompt_block();
        }
        let name = self.name.trim();
        if name.is_empty() && self.offer.trim().is_empty() {
            return None;
        }
        let mut out = String::new();
        if !name.is_empty() {
            out.push_str(&format!("Säljaren heter {name}. Skriv om säljaren som \"{}\".", self.pronoun()));
            if !self.company.trim().is_empty() {
                out.push_str(&format!(" {name} arbetar på {}.", self.company.trim()));
            }
            out.push_str(&format!(
                " Om någon i samtalet säger namnet {} är det sannolikt säljaren som presenterar sig.",
                self.first_name().unwrap_or(name)
            ));
        }
        if !self.offer.trim().is_empty() {
            out.push_str(&format!("\n\nDet här säljer säljaren, med egna ord:\n{}", self.offer.trim()));
        }
        Some(out)
    }
}

impl Profile {
    fn admin_prompt_block(&self) -> Option<String> {
        let name = self.name.trim();
        let company = self.company.trim();
        let policy = self.numbered_policy();
        let terms = self.terms.trim();
        if name.is_empty() && company.is_empty() && policy.is_empty() && terms.is_empty() {
            return None;
        }
        let mut out = String::new();
        if !name.is_empty() {
            out.push_str(&format!(
                "Granskningen görs åt {name}{}, som granskar andras samtal och inte deltar i det här. \
Säljaren i samtalet är någon annan, så använd inte namnet {name} för säljaren.",
                if company.is_empty() { String::new() } else { format!(" på {company}") }
            ));
        } else if !company.is_empty() {
            out.push_str(&format!("Säljaren arbetar på {company}."));
        }
        if !policy.is_empty() {
            out.push_str(&format!("\n\nFöretagets regler för säljsamtal:\n{policy}"));
        }
        if !terms.is_empty() {
            out.push_str(&format!("\n\nFöretagets allmänna villkor:\n<allmänna_villkor>\n{terms}\n</allmänna_villkor>"));
        }
        Some(out.trim().to_string())
    }
}

/// "Säljare" / "Kund" per segment id, from the AI's analysis. Stereo calls map whole
/// channels; mono calls map single lines.
pub fn roles(segments: &[Segment], tips: Option<&Tips>) -> Vec<Option<String>> {
    let mut out = vec![None; segments.len()];
    let Some(tips) = tips else { return out };
    for s in segments {
        let role = match &s.speaker {
            Some(sp) => tips.channel_roles.iter().find(|r| &r.speaker == sp).map(|r| r.role.clone()),
            None => tips.speakers.iter().find(|r| r.segment_id == s.id).map(|r| r.role.clone()),
        };
        if let Some(slot) = out.get_mut(s.id) {
            *slot = role;
        }
    }
    out
}

/// The name shown for each line: the seller's own name, the customer's name when the AI
/// found it in the call, else the role, else the channel ("Talare 1").
pub fn labels(segments: &[Segment], roles: &[Option<String>], profile: &Profile, tips: Option<&Tips>) -> Vec<Option<String>> {
    let customer = tips.map(|t| t.customer_name.trim()).filter(|n| !n.is_empty());
    // A review is of someone else's call: the seller is whoever the call names, never you.
    let seller = match tips {
        Some(t) if t.kind == Kind::Review => t.seller_name.split_whitespace().next(),
        _ => profile.first_name(),
    };
    segments
        .iter()
        .map(|s| match roles.get(s.id).and_then(|r| r.as_deref()) {
            Some(ROLE_SELLER) => Some(seller.unwrap_or(ROLE_SELLER).to_string()),
            Some(ROLE_CUSTOMER) => Some(customer.unwrap_or(ROLE_CUSTOMER).to_string()),
            _ => s.speaker.clone(),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CallStats {
    /// Share of the speaking time that was the seller, 0..=1.
    pub seller_talk_share: f32,
    pub seller_questions: usize,
    pub customer_questions: usize,
    /// Longest stretch the seller talked without the customer saying anything.
    pub longest_monologue_ms: u64,
    /// Whether the roles come from separate channels (exact) or the AI's guess (mono).
    pub exact: bool,
}

/// Talk statistics, once we know who is who. Counted locally from the transcript.
pub fn stats(segments: &[Segment], roles: &[Option<String>]) -> Option<CallStats> {
    let role = |s: &Segment| roles.get(s.id).and_then(|r| r.as_deref());
    let (mut seller_ms, mut customer_ms) = (0u64, 0u64);
    let (mut seller_q, mut customer_q) = (0usize, 0usize);
    let mut longest = 0u64;
    let mut run: Option<(u64, u64)> = None; // current seller stretch (start, end)
    for s in segments {
        let len = s.end_ms.saturating_sub(s.start_ms);
        let questions = s.text.matches('?').count();
        match role(s) {
            Some(ROLE_SELLER) => {
                seller_ms += len;
                seller_q += questions;
                run = Some(match run {
                    Some((start, _)) => (start, s.end_ms),
                    None => (s.start_ms, s.end_ms),
                });
                if let Some((start, end)) = run {
                    longest = longest.max(end.saturating_sub(start));
                }
            }
            Some(ROLE_CUSTOMER) => {
                customer_ms += len;
                customer_q += questions;
                run = None;
            }
            _ => {}
        }
    }
    if seller_ms == 0 || customer_ms == 0 {
        return None;
    }
    Some(CallStats {
        seller_talk_share: seller_ms as f32 / (seller_ms + customer_ms) as f32,
        seller_questions: seller_q,
        customer_questions: customer_q,
        longest_monologue_ms: longest,
        exact: segments.iter().any(|s| s.speaker.is_some()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tips::{ChannelRole, SegmentRole};

    fn seg(id: usize, start: u64, end: u64, text: &str, speaker: Option<&str>) -> Segment {
        Segment { id, start_ms: start, end_ms: end, text: text.into(), speaker: speaker.map(Into::into) }
    }

    fn tips(customer: &str) -> Tips {
        serde_json::from_value(serde_json::json!({
            "speakers": [], "channelRoles": [], "summary": "x", "sellingPoints": [], "customerNeeds": [],
            "objections": [], "missedOpportunities": [], "nextSteps": [], "customerName": customer, "customerCompany": ""
        }))
        .unwrap()
    }

    fn profile() -> Profile {
        Profile { name: "Anna Berg".into(), gender: "kvinna".into(), company: "Prospex".into(), ..Default::default() }
    }

    #[test]
    fn stereo_channels_get_names() {
        let segs = vec![seg(0, 0, 2_000, "Hej, det är Anna.", Some("Talare 1")), seg(1, 2_500, 4_000, "Hej, Erik här.", Some("Talare 2"))];
        let mut t = tips("Erik");
        t.channel_roles = vec![
            ChannelRole { speaker: "Talare 1".into(), role: ROLE_SELLER.into() },
            ChannelRole { speaker: "Talare 2".into(), role: ROLE_CUSTOMER.into() },
        ];
        let r = roles(&segs, Some(&t));
        assert_eq!(labels(&segs, &r, &profile(), Some(&t)), vec![Some("Anna".into()), Some("Erik".into())]);
    }

    #[test]
    fn mono_falls_back_to_roles_without_names() {
        let segs = vec![seg(0, 0, 1_000, "Hej!", None), seg(1, 1_000, 2_000, "Hallå?", None), seg(2, 2_000, 3_000, "Mm.", None)];
        let mut t = tips("");
        t.speakers = vec![
            SegmentRole { segment_id: 0, role: ROLE_SELLER.into() },
            SegmentRole { segment_id: 1, role: ROLE_CUSTOMER.into() },
        ];
        let r = roles(&segs, Some(&t));
        let l = labels(&segs, &r, &Profile::default(), Some(&t));
        assert_eq!(l, vec![Some("Säljare".into()), Some("Kund".into()), None]);
    }

    #[test]
    fn no_tips_keeps_channel_labels() {
        let segs = vec![seg(0, 0, 1_000, "Hej!", Some("Talare 1"))];
        let r = roles(&segs, None);
        assert_eq!(labels(&segs, &r, &profile(), None), vec![Some("Talare 1".into())]);
    }

    #[test]
    fn stats_count_talk_share_questions_and_monologue() {
        let segs = vec![
            seg(0, 0, 10_000, "Hej! Har du tid?", Some("Talare 1")),
            seg(1, 10_000, 30_000, "Lite grann. Vad gäller det?", Some("Talare 2")),
            seg(2, 30_000, 40_000, "Vi hjälper företag.", Some("Talare 1")),
            seg(3, 40_000, 55_000, "Med uppföljning. Hur gör ni idag?", Some("Talare 1")),
        ];
        let mut t = tips("");
        t.channel_roles = vec![
            ChannelRole { speaker: "Talare 1".into(), role: ROLE_SELLER.into() },
            ChannelRole { speaker: "Talare 2".into(), role: ROLE_CUSTOMER.into() },
        ];
        let st = stats(&segs, &roles(&segs, Some(&t))).unwrap();
        assert!((st.seller_talk_share - 35.0 / 55.0).abs() < 0.001);
        assert_eq!((st.seller_questions, st.customer_questions), (2, 1));
        assert_eq!(st.longest_monologue_ms, 25_000);
        assert!(st.exact);
    }

    #[test]
    fn stats_need_both_sides() {
        let segs = vec![seg(0, 0, 1_000, "Hej", None)];
        assert!(stats(&segs, &roles(&segs, None)).is_none());
    }

    #[test]
    fn profile_prompt_mentions_name_pronoun_and_offer() {
        let mut p = profile();
        p.offer = "CRM för hantverkare.".into();
        let text = p.prompt_block().unwrap();
        assert!(text.contains("Säljaren heter Anna Berg"));
        assert!(text.contains("\"hon\""));
        assert!(text.contains("arbetar på Prospex"));
        assert!(text.contains("CRM för hantverkare."));
        assert!(Profile::default().prompt_block().is_none());
    }

    fn admin() -> Profile {
        Profile {
            name: "Eva Lind".into(),
            company: "Prospex".into(),
            role: ROLE_ADMIN.into(),
            policy: "Nämn priset tidigt\n- Inga falska löften\n\n7. Fråga om inspelning".into(),
            ..Default::default()
        }
    }

    #[test]
    fn policy_is_numbered_once() {
        assert_eq!(admin().numbered_policy(), "1. Nämn priset tidigt\n2. Inga falska löften\n7. Fråga om inspelning");
    }

    #[test]
    fn admin_prompt_names_the_reviewer_and_the_rules() {
        let block = admin().prompt_block().unwrap();
        assert!(block.contains("Eva Lind på Prospex"));
        assert!(block.contains("använd inte namnet Eva Lind för säljaren"));
        assert!(block.contains("2. Inga falska löften"));
        assert!(!profile().prompt_block().unwrap().contains("regler"), "sellers get no rules");
    }

    #[test]
    fn reviews_never_label_the_admin_as_the_seller() {
        let segs = vec![seg(0, 0, 2_000, "Hej, det är Anna.", Some("Talare 1")), seg(1, 2_500, 4_000, "Hej.", Some("Talare 2"))];
        let mut t = tips("");
        t.kind = Kind::Review;
        t.channel_roles = vec![
            ChannelRole { speaker: "Talare 1".into(), role: ROLE_SELLER.into() },
            ChannelRole { speaker: "Talare 2".into(), role: ROLE_CUSTOMER.into() },
        ];
        let r = roles(&segs, Some(&t));
        assert_eq!(labels(&segs, &r, &admin(), Some(&t)), vec![Some("Säljare".into()), Some("Kund".into())]);
        t.seller_name = "Anna Berg".into();
        assert_eq!(labels(&segs, &r, &admin(), Some(&t))[0], Some("Anna".into()));
    }
}
