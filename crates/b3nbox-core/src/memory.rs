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
                embedding BLOB,
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
        self.add_with_embedding(kind, key, content, None)
    }

    pub fn add_with_embedding(
        &self,
        kind: &str,
        key: &str,
        content: &str,
        embedding: Option<&[f32]>,
    ) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let blob = embedding.map(crate::embeddings::vec_to_blob);
        conn.execute(
            "INSERT INTO memories (kind, key, content, embedding) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![kind, key, content, blob],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn search_similar(&self, query: &[f32], limit: u32) -> Result<Vec<crate::embeddings::Embedded>, String> {
        use crate::embeddings::{blob_to_vec, cosine_similarity, Embedded};

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, key, content, embedding, created_at FROM memories
                 WHERE embedding IS NOT NULL",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut results: Vec<Embedded> = rows
            .into_iter()
            .map(|(id, kind, key, content, blob, created_at)| {
                let similarity = cosine_similarity(query, &blob_to_vec(&blob));
                Embedded {
                    id,
                    kind,
                    key,
                    content,
                    created_at,
                    similarity,
                }
            })
            .collect();
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(limit as usize);
        Ok(results)
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<Memory>, String> {
        let pattern = format!("%{}%", query.replace(['%', '_'], ""));
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

    #[test]
    fn semantic_search_orders_by_similarity() {
        let mem = MemoryLayer::open(":memory:").unwrap();
        let v_a = vec![1.0, 0.0, 0.0];
        let v_b = vec![0.0, 1.0, 0.0];
        mem.add_with_embedding("doc", "a", "Dokument A", Some(&v_a)).unwrap();
        mem.add_with_embedding("doc", "b", "Dokument B", Some(&v_b)).unwrap();
        mem.add("doc", "c", "Ohne Embedding").unwrap();

        let hits = mem.search_similar(&v_a, 5).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].key, "a");
        assert!(hits[0].similarity > 0.99);
        assert_eq!(hits[1].key, "b");
        assert!(hits[1].similarity < 0.01);
    }
}
