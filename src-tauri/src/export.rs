//! "Exportera": the call as .txt, .md or .docx (tips first, then the transcript).

use std::path::Path;

use docx_rs::{Docx, Paragraph, Run};

use crate::db::Call;
use crate::tips::format_tips_plain;
use crate::transcript::{format_for_copy, format_timestamp, label_for};

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Txt,
    Md,
    Docx,
}

fn labels(call: &Call) -> &[Option<String>] {
    &call.labels
}

pub fn to_txt(call: &Call) -> String {
    let mut out = String::new();
    if let Some(tips) = &call.tips {
        out.push_str(&format_tips_plain(tips, &call.title));
        out.push_str("\n\n");
    } else {
        out.push_str(&format!("{}\n\n", call.title));
    }
    out.push_str("Transkribering\n");
    out.push_str(&format_for_copy(&call.segments, labels(call)));
    out.push('\n');
    out
}

pub fn to_markdown(call: &Call) -> String {
    let mut out = format!("# {}\n\n", call.title);
    if let Some(t) = &call.tips {
        out.push_str(&format!("## Sammanfattning\n\n{}\n\n", t.summary.trim()));
        if !t.selling_points.is_empty() {
            out.push_str("## Det här fungerade\n\n");
            for p in &t.selling_points {
                out.push_str(&format!(
                    "- **{}** – {}\n  > [{}] {}\n",
                    p.title, p.explanation, p.evidence.timestamp, p.evidence.quote
                ));
            }
            out.push('\n');
        }
        if !t.customer_needs.is_empty() {
            out.push_str("## Kundens behov\n\n");
            for p in &t.customer_needs {
                out.push_str(&format!("- {}\n  > [{}] {}\n", p.need, p.evidence.timestamp, p.evidence.quote));
            }
            out.push('\n');
        }
        if !t.objections.is_empty() {
            out.push_str("## Invändningar\n\n");
            for p in &t.objections {
                out.push_str(&format!(
                    "- **{}**\n  > [{}] {}\n\n  Förslag: {}\n",
                    p.objection, p.evidence.timestamp, p.evidence.quote, p.suggested_answer
                ));
            }
            out.push('\n');
        }
        if !t.missed_opportunities.is_empty() {
            out.push_str("## Missade möjligheter\n\n");
            for p in &t.missed_opportunities {
                out.push_str(&format!("- {} Förslag: {}\n", p.description, p.suggestion));
            }
            out.push('\n');
        }
        if !t.next_steps.is_empty() {
            out.push_str("## Nästa steg\n\n");
            for s in &t.next_steps {
                out.push_str(&format!("- {s}\n"));
            }
            out.push('\n');
        }
    }
    out.push_str("## Transkribering\n\n");
    let roles = labels(call);
    for s in &call.segments {
        match label_for(s, roles) {
            Some(l) => out.push_str(&format!("`[{}]` **{}:** {}  \n", format_timestamp(s.start_ms), l, s.text)),
            None => out.push_str(&format!("`[{}]` {}  \n", format_timestamp(s.start_ms), s.text)),
        }
    }
    out
}

fn heading(text: &str, size: usize) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text).bold().size(size))
}

fn para(text: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text))
}

fn quote(ts: &str, text: &str) -> Paragraph {
    Paragraph::new()
        .add_run(Run::new().add_text(format!("[{ts}] ")).color("666666"))
        .add_run(Run::new().add_text(format!("”{text}”")).italic())
}

pub fn write_docx(call: &Call, path: &Path) -> Result<(), String> {
    let mut doc = Docx::new().add_paragraph(heading(&call.title, 36));
    if let Some(t) = &call.tips {
        doc = doc.add_paragraph(heading("Sammanfattning", 28)).add_paragraph(para(t.summary.trim()));
        if !t.selling_points.is_empty() {
            doc = doc.add_paragraph(heading("Det här fungerade", 28));
            for p in &t.selling_points {
                doc = doc
                    .add_paragraph(Paragraph::new().add_run(Run::new().add_text(&p.title).bold()))
                    .add_paragraph(para(&p.explanation))
                    .add_paragraph(quote(&p.evidence.timestamp, &p.evidence.quote));
            }
        }
        if !t.customer_needs.is_empty() {
            doc = doc.add_paragraph(heading("Kundens behov", 28));
            for p in &t.customer_needs {
                doc = doc.add_paragraph(para(&format!("• {}", p.need))).add_paragraph(quote(&p.evidence.timestamp, &p.evidence.quote));
            }
        }
        if !t.objections.is_empty() {
            doc = doc.add_paragraph(heading("Invändningar", 28));
            for p in &t.objections {
                doc = doc
                    .add_paragraph(Paragraph::new().add_run(Run::new().add_text(&p.objection).bold()))
                    .add_paragraph(quote(&p.evidence.timestamp, &p.evidence.quote))
                    .add_paragraph(para(&format!("Förslag: {}", p.suggested_answer)));
            }
        }
        if !t.missed_opportunities.is_empty() {
            doc = doc.add_paragraph(heading("Missade möjligheter", 28));
            for p in &t.missed_opportunities {
                doc = doc.add_paragraph(para(&format!("• {} Förslag: {}", p.description, p.suggestion)));
            }
        }
        if !t.next_steps.is_empty() {
            doc = doc.add_paragraph(heading("Nästa steg", 28));
            for s in &t.next_steps {
                doc = doc.add_paragraph(para(&format!("• {s}")));
            }
        }
    }
    doc = doc.add_paragraph(heading("Transkribering", 28));
    let roles = labels(call);
    for s in &call.segments {
        let mut p = Paragraph::new().add_run(Run::new().add_text(format!("[{}] ", format_timestamp(s.start_ms))).color("666666"));
        if let Some(l) = label_for(s, roles) {
            p = p.add_run(Run::new().add_text(format!("{l}: ")).bold());
        }
        doc = doc.add_paragraph(p.add_run(Run::new().add_text(&s.text)));
    }
    let file = std::fs::File::create(path).map_err(|e| format!("Kunde inte skapa filen: {e}"))?;
    doc.build().pack(file).map_err(|e| format!("Kunde inte skriva Word-filen: {e}"))
}

pub fn export(call: &Call, format: Format, path: &Path) -> Result<(), String> {
    let write = |s: String| std::fs::write(path, s).map_err(|e| format!("Kunde inte spara filen: {e}"));
    match format {
        Format::Txt => write(to_txt(call)),
        Format::Md => write(to_markdown(call)),
        Format::Docx => write_docx(call, path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Segment;

    fn call() -> Call {
        Call {
            id: "x".into(),
            title: "Demo med Kund AB".into(),
            file_path: "/tmp/x.wav".into(),
            file_name: "x.wav".into(),
            created_at: "2026-10-27T10:00:00Z".into(),
            duration_ms: 10_000,
            channels: 2,
            model: "latt".into(),
            status: "done".into(),
            error: None,
            segments: vec![
                Segment { id: 0, start_ms: 0, end_ms: 2_000, text: "Hej!".into(), speaker: Some("Talare 1".into()) },
                Segment { id: 1, start_ms: 3_000, end_ms: 5_000, text: "Hallå.".into(), speaker: Some("Talare 2".into()) },
            ],
            tips: None,
            roles: vec![],
            labels: vec![],
            stats: None,
        }
    }

    #[test]
    fn txt_contains_transcript_in_copy_format() {
        let txt = to_txt(&call());
        assert!(txt.contains("Transkribering\n[00:00] Talare 1: Hej!\n[00:03] Talare 2: Hallå.\n"));
    }

    #[test]
    fn markdown_has_title_and_lines() {
        let md = to_markdown(&call());
        assert!(md.starts_with("# Demo med Kund AB\n"));
        assert!(md.contains("`[00:03]` **Talare 2:** Hallå."));
    }

    #[test]
    fn docx_is_a_zip_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.docx");
        export(&call(), Format::Docx, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }
}
