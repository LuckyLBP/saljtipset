//! Local SQLite store for calls (transcript + tips) and settings. Audio is never stored.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::tips::Tips;
use crate::transcript::Segment;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: i64,
    /// "user" or "assistant"
    pub role: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CallSummary {
    pub id: String,
    pub title: String,
    pub file_name: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub status: String,
    pub has_tips: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Call {
    pub id: String,
    pub title: String,
    pub file_path: String,
    pub file_name: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub channels: usize,
    pub model: String,
    /// queued | transcribing | done | error
    pub status: String,
    pub error: Option<String>,
    pub segments: Vec<Segment>,
    pub tips: Option<Tips>,
    /// Computed when the call is loaded, never stored: who said each line, the name to show
    /// for it, and talk statistics (see people.rs).
    #[serde(default, skip_deserializing)]
    pub roles: Vec<Option<String>>,
    #[serde(default, skip_deserializing)]
    pub labels: Vec<Option<String>>,
    #[serde(default, skip_deserializing)]
    pub stats: Option<crate::people::CallStats>,
}

pub struct Db {
    conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS calls (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    file_name   TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    channels    INTEGER NOT NULL DEFAULT 1,
    model       TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL,
    error       TEXT,
    segments    TEXT NOT NULL DEFAULT '[]',
    tips        TEXT
);
CREATE INDEX IF NOT EXISTS calls_created ON calls(created_at DESC);
CREATE TABLE IF NOT EXISTS chat_messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    call_id    TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
    role       TEXT NOT NULL,
    text       TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS chat_by_call ON chat_messages(call_id, id);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        // A crash or quit mid-transcription leaves rows stuck in "transcribing" or "queued".
        conn.execute(
            "UPDATE calls SET status = 'error', error = 'Transkriberingen avbröts när appen stängdes.' WHERE status = 'transcribing'",
            [],
        )?;
        conn.execute(
            "UPDATE calls SET status = 'error', error = 'Samtalet låg i kön när appen stängdes.' WHERE status = 'queued'",
            [],
        )?;
        Ok(Self { conn })
    }

    pub fn insert_call(&self, call: &Call) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO calls (id, title, file_path, file_name, created_at, duration_ms, channels, model, status, error, segments, tips)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                call.id,
                call.title,
                call.file_path,
                call.file_name,
                call.created_at,
                call.duration_ms as i64,
                call.channels as i64,
                call.model,
                call.status,
                call.error,
                serde_json::to_string(&call.segments).unwrap(),
                call.tips.as_ref().map(|t| serde_json::to_string(t).unwrap()),
            ],
        )?;
        Ok(())
    }

    pub fn finish_transcription(
        &self,
        id: &str,
        segments: &[Segment],
        duration_ms: u64,
        channels: usize,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE calls SET status = 'done', error = NULL, segments = ?2, duration_ms = ?3, channels = ?4 WHERE id = ?1",
            params![id, serde_json::to_string(segments).unwrap(), duration_ms as i64, channels as i64],
        )?;
        Ok(())
    }

    pub fn set_status(&self, id: &str, status: &str, error: Option<&str>) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE calls SET status = ?2, error = ?3 WHERE id = ?1", params![id, status, error])?;
        Ok(())
    }

    pub fn save_tips(&self, id: &str, tips: &Tips) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE calls SET tips = ?2 WHERE id = ?1", params![id, serde_json::to_string(tips).unwrap()])?;
        Ok(())
    }

    pub fn rename(&self, id: &str, title: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE calls SET title = ?2 WHERE id = ?1", params![id, title])?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM calls WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list(&self) -> rusqlite::Result<Vec<CallSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, file_name, created_at, duration_ms, status, tips IS NOT NULL FROM calls ORDER BY created_at DESC",
        )?;
        stmt.query_map([], |r| {
            Ok(CallSummary {
                id: r.get(0)?,
                title: r.get(1)?,
                file_name: r.get(2)?,
                created_at: r.get(3)?,
                duration_ms: r.get::<_, i64>(4)? as u64,
                status: r.get(5)?,
                has_tips: r.get(6)?,
            })
        })?
        .collect()
    }

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<Call>> {
        self.conn
            .query_row(
                "SELECT id, title, file_path, file_name, created_at, duration_ms, channels, model, status, error, segments, tips
                 FROM calls WHERE id = ?1",
                params![id],
                |r| {
                    let segments: String = r.get(10)?;
                    let tips: Option<String> = r.get(11)?;
                    Ok(Call {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        file_path: r.get(2)?,
                        file_name: r.get(3)?,
                        created_at: r.get(4)?,
                        duration_ms: r.get::<_, i64>(5)? as u64,
                        channels: r.get::<_, i64>(6)? as usize,
                        model: r.get(7)?,
                        status: r.get(8)?,
                        error: r.get(9)?,
                        segments: serde_json::from_str(&segments).unwrap_or_default(),
                        tips: tips.and_then(|t| serde_json::from_str(&t).ok()),
                        roles: Vec::new(),
                        labels: Vec::new(),
                        stats: None,
                    })
                },
            )
            .optional()
    }

    pub fn delete_all(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch("DELETE FROM chat_messages; DELETE FROM calls; DELETE FROM settings; VACUUM;")
    }

    pub fn chat_history(&self, call_id: &str) -> rusqlite::Result<Vec<ChatMessage>> {
        let mut stmt =
            self.conn.prepare("SELECT id, role, text, created_at FROM chat_messages WHERE call_id = ?1 ORDER BY id")?;
        stmt.query_map(params![call_id], |r| {
            Ok(ChatMessage { id: r.get(0)?, role: r.get(1)?, text: r.get(2)?, created_at: r.get(3)? })
        })?
        .collect()
    }

    /// Store a question and its answer together, so a failed answer leaves no dangling question.
    pub fn add_chat_exchange(&self, call_id: &str, question: &str, answer: &str) -> rusqlite::Result<Vec<ChatMessage>> {
        let now = chrono::Utc::now().to_rfc3339();
        let tx = self.conn.unchecked_transaction()?;
        for (role, text) in [("user", question), ("assistant", answer)] {
            tx.execute(
                "INSERT INTO chat_messages (call_id, role, text, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![call_id, role, text, now],
            )?;
        }
        tx.commit()?;
        self.chat_history(call_id)
    }

    pub fn clear_chat(&self, call_id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM chat_messages WHERE call_id = ?1", params![call_id])?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0)).optional().ok()?
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(id: &str, created_at: &str) -> Call {
        Call {
            id: id.into(),
            title: "Samtal".into(),
            file_path: "/tmp/a.m4a".into(),
            file_name: "a.m4a".into(),
            created_at: created_at.into(),
            duration_ms: 0,
            channels: 1,
            model: "latt".into(),
            status: "transcribing".into(),
            error: None,
            segments: vec![],
            tips: None,
            roles: vec![],
            labels: vec![],
            stats: None,
        }
    }

    #[test]
    fn call_lifecycle() {
        let db = Db::open_in_memory().unwrap();
        db.insert_call(&call("a", "2026-10-01T10:00:00Z")).unwrap();
        db.insert_call(&call("b", "2026-10-02T10:00:00Z")).unwrap();
        let segs = vec![Segment { id: 0, start_ms: 0, end_ms: 900, text: "Hej".into(), speaker: None }];
        db.finish_transcription("a", &segs, 90_000, 2).unwrap();
        db.rename("a", "Kund AB").unwrap();

        let list = db.list().unwrap();
        assert_eq!(list.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
        let a = db.get("a").unwrap().unwrap();
        assert_eq!((a.title.as_str(), a.status.as_str(), a.channels, a.duration_ms), ("Kund AB", "done", 2, 90_000));
        assert_eq!(a.segments, segs);

        db.add_chat_exchange("a", "Hur gick det?", "Bra!").unwrap();
        let chat = db.add_chat_exchange("a", "Och priset?", "Det var ett hinder.").unwrap();
        assert_eq!(chat.iter().map(|m| m.role.as_str()).collect::<Vec<_>>(), vec!["user", "assistant", "user", "assistant"]);
        assert_eq!(chat[3].text, "Det var ett hinder.");

        db.delete("a").unwrap();
        assert!(db.get("a").unwrap().is_none());
        assert!(db.chat_history("a").unwrap().is_empty(), "chat goes with the call");
        db.delete_all().unwrap();
        assert!(db.list().unwrap().is_empty());
    }

    #[test]
    fn settings_upsert() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.get_setting("model"), None);
        db.set_setting("model", "bast").unwrap();
        db.set_setting("model", "latt").unwrap();
        assert_eq!(db.get_setting("model").as_deref(), Some("latt"));
    }
}
