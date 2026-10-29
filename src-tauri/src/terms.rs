//! "Allmänna villkor": the company's terms, uploaded by an admin, as plain text for reviews.
//!
//! The file is read once on upload (PDF, Word or text) and only its text is kept, in the
//! settings table, so it goes with "Radera all data". Reviews and the reviewer chat get it
//! through [`crate::people::Profile::terms`].

use std::path::Path;

use serde::{Deserialize, Serialize};

/// About 15k tokens. Longer terms are cut, and the UI says so.
pub const MAX_CHARS: usize = 60_000;

pub const EXTENSIONS: [&str; 4] = ["pdf", "docx", "txt", "md"];

/// What is stored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Terms {
    pub file_name: String,
    pub text: String,
    pub truncated: bool,
    pub imported_at: String,
}

/// What the UI sees: everything but the text.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TermsInfo {
    pub file_name: String,
    pub chars: usize,
    pub truncated: bool,
    pub imported_at: String,
}

impl Terms {
    pub fn info(&self) -> TermsInfo {
        TermsInfo {
            file_name: self.file_name.clone(),
            chars: self.text.chars().count(),
            truncated: self.truncated,
            imported_at: self.imported_at.clone(),
        }
    }
}

/// Read a terms file into [`Terms`].
pub fn import(path: &Path) -> Result<Terms, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
    let raw = match ext.as_str() {
        "pdf" => pdf_text(path)?,
        "docx" => docx_text(path)?,
        "txt" | "md" => {
            let bytes = std::fs::read(path).map_err(|e| format!("Kunde inte läsa filen: {e}"))?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        _ => return Err("Välj en PDF, ett Word-dokument (.docx) eller en textfil.".into()),
    };
    let text = tidy(&raw);
    if text.chars().filter(|c| c.is_alphanumeric()).count() < 50 {
        return Err(if ext == "pdf" {
            "Hittade ingen text i PDF:en. Är den inskannad? Spara villkoren som Word eller text och försök igen.".into()
        } else {
            "Hittade ingen text i filen.".into()
        });
    }
    let truncated = text.chars().count() > MAX_CHARS;
    let text = if truncated { text.chars().take(MAX_CHARS).collect() } else { text };
    Ok(Terms {
        file_name: path.file_name().and_then(|n| n.to_str()).unwrap_or("villkor").to_string(),
        text,
        truncated,
        imported_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn pdf_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Kunde inte läsa filen: {e}"))?;
    // pdf-extract panics on some malformed PDFs; treat that like any unreadable file.
    std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes))
        .map_err(|_| "PDF:en gick inte att läsa. Spara villkoren som Word eller text och försök igen.".to_string())?
        .map_err(|e| format!("PDF:en gick inte att läsa ({e}). Spara villkoren som Word eller text och försök igen."))
}

fn docx_text(path: &Path) -> Result<String, String> {
    use docx_rs::{DocumentChild, TableCellContent, TableChild, TableRowChild};
    let bytes = std::fs::read(path).map_err(|e| format!("Kunde inte läsa filen: {e}"))?;
    let doc = docx_rs::read_docx(&bytes).map_err(|e| format!("Word-filen gick inte att läsa ({e})."))?;
    let mut out = String::new();
    for child in &doc.document.children {
        match child {
            DocumentChild::Paragraph(p) => {
                out.push_str(&p.raw_text());
                out.push('\n');
            }
            // Terms often put clauses in tables: one line per row, cells separated.
            DocumentChild::Table(t) => {
                for TableChild::TableRow(row) in &t.rows {
                    let cells: Vec<String> = row
                        .cells
                        .iter()
                        .map(|TableRowChild::TableCell(cell)| {
                            cell.children
                                .iter()
                                .filter_map(|c| match c {
                                    TableCellContent::Paragraph(p) => Some(p.raw_text()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect();
                    out.push_str(&cells.join(" | "));
                    out.push('\n');
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Trim lines, collapse runs of spaces and blank lines, so the AI gets dense text.
fn tidy(raw: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in raw.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(if blank > 0 { "\n\n" } else { "\n" });
        }
        out.push_str(&line);
        blank = 0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUSE: &str = "Avtalet löper i 12 månader och förlängs med 12 månader i taget om det inte sägs upp senast tre månader före avtalstidens slut.";

    #[test]
    fn tidy_collapses_whitespace() {
        assert_eq!(tidy("  §1   Avtal \n\n\n\n  gäller  \r\n"), "§1 Avtal\n\ngäller");
    }

    #[test]
    fn reads_text_and_word_files() {
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("villkor.txt");
        std::fs::write(&txt, format!("§ 4 Avtalstid\n\n{CLAUSE}")).unwrap();
        let t = import(&txt).unwrap();
        assert_eq!(t.file_name, "villkor.txt");
        assert!(t.text.starts_with("§ 4 Avtalstid\n\n"));
        assert!(!t.truncated);

        let docx = dir.path().join("villkor.docx");
        let file = std::fs::File::create(&docx).unwrap();
        docx_rs::Docx::new()
            .add_paragraph(docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text("§ 4 Avtalstid")))
            .add_paragraph(docx_rs::Paragraph::new().add_run(docx_rs::Run::new().add_text(CLAUSE)))
            .build()
            .pack(file)
            .unwrap();
        let t = import(&docx).unwrap();
        assert_eq!(t.text, format!("§ 4 Avtalstid\n{CLAUSE}"));
    }

    #[test]
    fn reads_a_pdf() {
        // A one-page sample made with macOS cupsfilter from plain text.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/villkor.pdf");
        let t = import(&path).unwrap();
        assert!(t.text.contains("§ 1 Pris"), "{}", t.text);
        assert!(t.text.contains("450 kronor per användare och månad"));
        assert!(t.text.contains("senast tre månader före avtalstidens slut"));
    }

    #[test]
    fn rejects_empty_and_unknown_files_and_cuts_long_ones() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("tom.txt");
        std::fs::write(&empty, "   \n").unwrap();
        assert!(import(&empty).unwrap_err().contains("ingen text"));
        let odd = dir.path().join("villkor.xls");
        std::fs::write(&odd, CLAUSE).unwrap();
        assert!(import(&odd).unwrap_err().contains("PDF"));
        let long = dir.path().join("lang.txt");
        std::fs::write(&long, CLAUSE.repeat(600)).unwrap();
        let t = import(&long).unwrap();
        assert!(t.truncated);
        assert_eq!(t.text.chars().count(), MAX_CHARS);
    }
}
