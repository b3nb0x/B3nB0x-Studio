use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub created_at: String,
    pub route: String,
    pub backend: String,
    pub model: String,
    pub duration_ms: u64,
    pub status: String,
    pub prompt_chars: i64,
    pub response_chars: i64,
    pub prompt_hash: String,
}

pub struct AuditRecord<'a> {
    pub route: &'a str,
    pub backend: &'a str,
    pub model: &'a str,
    pub duration_ms: u64,
    pub status: &'a str,
    pub prompt: &'a str,
    pub response: &'a str,
}

pub struct AuditLog {
    conn: Mutex<Connection>,
}

impl AuditLog {
    pub fn open(path: &str) -> Result<Self, String> {
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                route TEXT NOT NULL,
                backend TEXT NOT NULL,
                model TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                status TEXT NOT NULL,
                prompt_chars INTEGER NOT NULL,
                response_chars INTEGER NOT NULL,
                prompt_hash TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_log(created_at);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn log(&self, record: AuditRecord<'_>) -> Result<(), String> {
        let mut hasher = DefaultHasher::new();
        record.prompt.hash(&mut hasher);
        let prompt_hash = format!("{:016x}", hasher.finish());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_log (route, backend, model, duration_ms, status, prompt_chars, response_chars, prompt_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                record.route,
                record.backend,
                record.model,
                record.duration_ms as i64,
                record.status,
                record.prompt.chars().count() as i64,
                record.response.chars().count() as i64,
                prompt_hash
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn recent(&self, limit: u32) -> Result<Vec<AuditEntry>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, created_at, route, backend, model, duration_ms, status,
                        prompt_chars, response_chars, prompt_hash
                 FROM audit_log ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![limit], |row| {
                Ok(AuditEntry {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    route: row.get(2)?,
                    backend: row.get(3)?,
                    model: row.get(4)?,
                    duration_ms: row.get::<_, i64>(5)? as u64,
                    status: row.get(6)?,
                    prompt_chars: row.get(7)?,
                    response_chars: row.get(8)?,
                    prompt_hash: row.get(9)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_and_recent() {
        let audit = AuditLog::open(":memory:").unwrap();
        audit
            .log(AuditRecord {
                route: "default",
                backend: "ollama",
                model: "llama3",
                duration_ms: 123,
                status: "ok",
                prompt: "Hallo Welt",
                response: "Hi!",
            })
            .unwrap();
        audit
            .log(AuditRecord {
                route: "analyse",
                backend: "ollama",
                model: "llama3",
                duration_ms: 456,
                status: "error",
                prompt: "Analyse...",
                response: "",
            })
            .unwrap();

        let entries = audit.recent(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].route, "analyse");
        assert_eq!(entries[0].status, "error");
        assert_eq!(entries[1].status, "ok");
        assert_eq!(entries[1].duration_ms, 123);
        assert_eq!(entries[1].response_chars, 3);
        assert_eq!(entries[1].prompt_hash.len(), 16);
    }
}
