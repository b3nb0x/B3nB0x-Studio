use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub kind: String,
    pub key: String,
    pub content: String,
    pub created_at: String,
}

pub struct MemoryLayer {
    conn: Mutex<Connection>,
}

impl MemoryLayer {
    pub fn open(path: &str) -> Result<Self, String> {
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                key TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories(kind);
            CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn add(&self, kind: &str, key: &str, content: &str) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO memories (kind, key, content) VALUES (?1, ?2, ?3)",
            rusqlite::params![kind, key, content],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<Memory>, String> {
        let pattern = format!("%{}%", query.replace('%', ""));
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, key, content, created_at FROM memories
                 WHERE content LIKE ?1 OR key LIKE ?1
                 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![pattern, limit], row_to_memory)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    pub fn recent(&self, limit: u32) -> Result<Vec<Memory>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, key, content, created_at FROM memories
                 ORDER BY id DESC LIMIT ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![limit], row_to_memory)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }
}

fn row_to_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<Memory> {
    Ok(Memory {
        id: row.get(0)?,
        kind: row.get(1)?,
        key: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_search_recent() {
        let mem = MemoryLayer::open(":memory:").unwrap();
        mem.add("project", "b3nbox", "B3nBox Studio ist ein Agent-Framework in Rust").unwrap();
        mem.add("task", "run-1", "Pipeline-Run für Log-Analyse Tool").unwrap();

        let hits = mem.search("Log-Analyse", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "task");

        let recent = mem.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].key, "run-1");
    }
}
