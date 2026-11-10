use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub message_count: i64,
}

pub struct SessionStore {
    conn: Mutex<Connection>,
}

impl SessionStore {
    pub fn open(path: &str) -> Result<Self, String> {
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS session_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            CREATE INDEX IF NOT EXISTS idx_session_messages_session ON session_messages(session_id);",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn create_session(&self, name: &str) -> Result<Session, String> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, name) VALUES (?1, ?2)",
            rusqlite::params![id, name],
        )
        .map_err(|e| e.to_string())?;
        Ok(Session {
            id,
            name: name.to_string(),
            created_at: String::new(),
            message_count: 0,
        })
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.name, s.created_at, COUNT(m.id)
                 FROM sessions s LEFT JOIN session_messages m ON m.session_id = s.id
                 GROUP BY s.id ORDER BY s.created_at DESC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Session {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    message_count: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    pub fn add_message(&self, session_id: &str, role: &str, content: &str) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO session_messages (session_id, role, content) VALUES (?1, ?2, ?3)",
            rusqlite::params![session_id, role, content],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn history(&self, session_id: &str) -> Result<Vec<SessionMessage>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, role, content, created_at FROM session_messages
                 WHERE session_id = ?1 ORDER BY id ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![session_id], |row| {
                Ok(SessionMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }

    pub fn delete_session(&self, session_id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM session_messages WHERE session_id = ?1", rusqlite::params![session_id])
            .map_err(|e| e.to_string())?;
        let deleted = conn
            .execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![session_id])
            .map_err(|e| e.to_string())?;
        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lifecycle() {
        let store = SessionStore::open(":memory:").unwrap();
        let session = store.create_session("Test-Chat").unwrap();
        assert!(!session.id.is_empty());

        store.add_message(&session.id, "user", "Hallo").unwrap();
        store.add_message(&session.id, "assistant", "Hi! Wie kann ich helfen?").unwrap();

        let history = store.history(&session.id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, "user");
        assert_eq!(history[1].content, "Hi! Wie kann ich helfen?");

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].message_count, 2);
        assert_eq!(sessions[0].name, "Test-Chat");

        assert!(store.delete_session(&session.id).unwrap());
        assert!(store.history(&session.id).unwrap().is_empty());
        assert!(store.list_sessions().unwrap().is_empty());
    }
}
