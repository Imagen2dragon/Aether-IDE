use application_core::{AetherError, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct DatabaseEngine {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRecord {
    pub id: Option<i64>,
    pub file_path: String,
    pub name: String,
    pub kind: String,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: String,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfWorkRecord {
    pub id: Option<i64>,
    pub task_id: String,
    pub step_index: usize,
    pub tool_name: String,
    pub input_json: String,
    pub output_json: String,
    pub diff: Option<String>,
    pub exit_code: i32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemoryNote {
    pub id: String,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl DatabaseEngine {
    pub fn open_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().map_err(|e| AetherError::Database(e.to_string()))?;
        let engine = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        engine.init_tables()?;
        Ok(engine)
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).map_err(|e| AetherError::Database(e.to_string()))?;

        // Enable WAL mode, memory cache, and synchronous=NORMAL
        let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()));
        let _ = conn.query_row("PRAGMA synchronous = NORMAL", [], |_| Ok(()));
        let _ = conn.query_row("PRAGMA foreign_keys = ON", [], |_| Ok(()));
        let _ = conn.query_row("PRAGMA cache_size = -64000", [], |_| Ok(()));

        let engine = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        engine.init_tables()?;
        Ok(engine)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS symbols (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                line_start INTEGER NOT NULL,
                line_end INTEGER NOT NULL,
                signature TEXT NOT NULL,
                docstring TEXT
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS symbols_fts USING fts5(
                name,
                signature,
                docstring,
                content='symbols',
                content_rowid='id'
            );

            CREATE TABLE IF NOT EXISTS proof_of_work (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                tool_name TEXT NOT NULL,
                input_json TEXT NOT NULL,
                output_json TEXT NOT NULL,
                diff TEXT,
                exit_code INTEGER NOT NULL,
                timestamp TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS project_brain_nodes (
                node_id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                node_type TEXT NOT NULL,
                metadata_json TEXT
            );

            CREATE TABLE IF NOT EXISTS project_brain_edges (
                source_node TEXT NOT NULL,
                target_node TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL DEFAULT 1.0,
                PRIMARY KEY (source_node, target_node, edge_type)
            );

            CREATE TABLE IF NOT EXISTS project_memory_notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                tags_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .map_err(|e| AetherError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn insert_symbol(&self, sym: &SymbolRecord) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO symbols (file_path, name, kind, line_start, line_end, signature, docstring)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                sym.file_path,
                sym.name,
                sym.kind,
                sym.line_start,
                sym.line_end,
                sym.signature,
                sym.docstring
            ],
        ).map_err(|e| AetherError::Database(e.to_string()))?;

        let id = conn.last_insert_rowid();

        // Update FTS5 virtual table
        let _ = conn.execute(
            "INSERT INTO symbols_fts (rowid, name, signature, docstring) VALUES (?1, ?2, ?3, ?4)",
            params![id, sym.name, sym.signature, sym.docstring],
        );

        Ok(id)
    }

    pub fn delete_file_symbols(&self, file_path: &str) -> Result<usize> {
        let conn = self.conn.lock();
        let rows = conn
            .execute(
                "DELETE FROM symbols WHERE file_path = ?1",
                params![file_path],
            )
            .map_err(|e| AetherError::Database(e.to_string()))?;
        Ok(rows)
    }

    pub fn search_symbols_fts(&self, query: &str, limit: usize) -> Result<Vec<SymbolRecord>> {
        let conn = self.conn.lock();
        let formatted_query = format!("{}*", query.replace('"', "\"\""));
        let mut stmt = conn.prepare(
            "SELECT s.id, s.file_path, s.name, s.kind, s.line_start, s.line_end, s.signature, s.docstring
             FROM symbols s
             JOIN symbols_fts f ON s.id = f.rowid
             WHERE symbols_fts MATCH ?1
             LIMIT ?2",
        ).map_err(|e| AetherError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![formatted_query, limit as i64], |row| {
                Ok(SymbolRecord {
                    id: Some(row.get(0)?),
                    file_path: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    line_start: row.get(4)?,
                    line_end: row.get(5)?,
                    signature: row.get(6)?,
                    docstring: row.get(7)?,
                })
            })
            .map_err(|e| AetherError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for sym in rows.flatten() {
            results.push(sym);
        }
        Ok(results)
    }

    pub fn record_proof_of_work(&self, pow: &ProofOfWorkRecord) -> Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO proof_of_work (task_id, step_index, tool_name, input_json, output_json, diff, exit_code, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                pow.task_id,
                pow.step_index as i64,
                pow.tool_name,
                pow.input_json,
                pow.output_json,
                pow.diff,
                pow.exit_code,
                pow.timestamp
            ],
        ).map_err(|e| AetherError::Database(e.to_string()))?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_proof_of_work(&self, task_id: &str) -> Result<Vec<ProofOfWorkRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, task_id, step_index, tool_name, input_json, output_json, diff, exit_code, timestamp
             FROM proof_of_work WHERE task_id = ?1 ORDER BY step_index ASC",
        ).map_err(|e| AetherError::Database(e.to_string()))?;

        let rows = stmt
            .query_map(params![task_id], |row| {
                Ok(ProofOfWorkRecord {
                    id: Some(row.get(0)?),
                    task_id: row.get(1)?,
                    step_index: row.get::<_, i64>(2)? as usize,
                    tool_name: row.get(3)?,
                    input_json: row.get(4)?,
                    output_json: row.get(5)?,
                    diff: row.get(6)?,
                    exit_code: row.get(7)?,
                    timestamp: row.get(8)?,
                })
            })
            .map_err(|e| AetherError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for rec in rows.flatten() {
            results.push(rec);
        }
        Ok(results)
    }

    pub fn save_memory_note(&self, note: &ProjectMemoryNote) -> Result<()> {
        let conn = self.conn.lock();
        let tags_json = serde_json::to_string(&note.tags).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "INSERT INTO project_memory_notes (id, title, content, tags_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET title=?2, content=?3, tags_json=?4, updated_at=?6",
            params![
                note.id,
                note.title,
                note.content,
                tags_json,
                note.created_at,
                note.updated_at
            ],
        )
        .map_err(|e| AetherError::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get_memory_notes(&self) -> Result<Vec<ProjectMemoryNote>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT id, title, content, tags_json, created_at, updated_at
                 FROM project_memory_notes ORDER BY updated_at DESC",
            )
            .map_err(|e| AetherError::Database(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(3)?;
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(ProjectMemoryNote {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    tags,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(|e| AetherError::Database(e.to_string()))?;

        let mut notes = Vec::new();
        for n in rows.flatten() {
            notes.push(n);
        }
        Ok(notes)
    }

    pub fn delete_memory_note(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let rows = conn
            .execute(
                "DELETE FROM project_memory_notes WHERE id = ?1",
                params![id],
            )
            .map_err(|e| AetherError::Database(e.to_string()))?;
        Ok(rows > 0)
    }

    /// Performs a non-blocking, consistent online backup of the SQLite database
    /// to the destination path using the native SQLite online backup API (`sqlite3_backup`).
    /// This works safely on live WAL databases without database lockups or inconsistent read states.
    pub fn backup_online<P: AsRef<Path>>(&self, dest_path: P) -> Result<()> {
        let dest = dest_path.as_ref();
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut dst_conn =
            Connection::open(dest).map_err(|e| AetherError::Database(e.to_string()))?;
        let src_conn = self.conn.lock();

        let backup = rusqlite::backup::Backup::new(&src_conn, &mut dst_conn).map_err(|e| {
            AetherError::Database(format!("Failed to initialize online backup: {e}"))
        })?;

        backup
            .run_to_completion(100, std::time::Duration::from_millis(10), None)
            .map_err(|e| AetherError::Database(format!("Backup execution failed: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_fts_symbol_search() {
        let db = DatabaseEngine::open_in_memory().expect("failed to open in-memory db");

        let sym1 = SymbolRecord {
            id: None,
            file_path: "src/auth/jwt.rs".into(),
            name: "validate_jwt_token".into(),
            kind: "function".into(),
            line_start: 12,
            line_end: 25,
            signature: "pub fn validate_jwt_token(token: &str) -> Result<Claims>".into(),
            docstring: Some("Validates the authenticity and expiration of incoming tokens.".into()),
        };

        db.insert_symbol(&sym1).expect("insert symbol failed");

        let search_results = db
            .search_symbols_fts("validate_jwt", 10)
            .expect("fts failed");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "validate_jwt_token");
    }

    #[test]
    fn test_proof_of_work_ledger() {
        let db = DatabaseEngine::open_in_memory().expect("failed to open in-memory db");

        let pow = ProofOfWorkRecord {
            id: None,
            task_id: "task-001".into(),
            step_index: 0,
            tool_name: "cargo_test".into(),
            input_json: r#"{"args": ["test"]}"#.into(),
            output_json: r#"{"passed": 12, "failed": 0}"#.into(),
            diff: None,
            exit_code: 0,
            timestamp: "2026-10-30T21:00:00Z".into(),
        };

        db.record_proof_of_work(&pow).expect("record failed");
        let ledger = db.get_proof_of_work("task-001").expect("read failed");
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].tool_name, "cargo_test");
        assert_eq!(ledger[0].exit_code, 0);
    }

    #[test]
    fn test_project_memory_crud() {
        let db = DatabaseEngine::open_in_memory().expect("failed to open in-memory db");

        let note = ProjectMemoryNote {
            id: "mem-01".into(),
            title: "Architecture Decision: ConPTY".into(),
            content: "We use Windows ConPTY with ScopedJobObject for reliable process containment."
                .into(),
            tags: vec!["architecture".into(), "terminal".into(), "security".into()],
            created_at: "2026-10-31T11:00:00Z".into(),
            updated_at: "2026-10-31T11:00:00Z".into(),
        };

        db.save_memory_note(&note).expect("save failed");
        let notes = db.get_memory_notes().expect("get failed");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "Architecture Decision: ConPTY");
        assert_eq!(notes[0].tags.len(), 3);

        let deleted = db.delete_memory_note("mem-01").expect("delete failed");
        assert!(deleted);
        assert_eq!(db.get_memory_notes().expect("get after delete").len(), 0);
    }

    #[test]
    fn test_sqlite_online_backup_and_restore() {
        let temp_dir =
            std::env::temp_dir().join(format!("aether_backup_test_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let live_db_path = temp_dir.join("live.db");
        let backup_db_path = temp_dir.join("backup.db");

        // 1. Open live database in WAL mode and insert records
        let live_db = DatabaseEngine::open(&live_db_path).expect("failed to open live db");
        let sym = SymbolRecord {
            id: None,
            file_path: "src/crypto.rs".into(),
            name: "chacha20_poly1305_encrypt".into(),
            kind: "function".into(),
            line_start: 45,
            line_end: 90,
            signature: "pub fn chacha20_poly1305_encrypt()".into(),
            docstring: Some("Encrypts payload using ChaCha20-Poly1305".into()),
        };
        live_db.insert_symbol(&sym).expect("failed to index symbol");

        let note = ProjectMemoryNote {
            id: "mem-backup-01".into(),
            title: "Backup Verification".into(),
            content: "Testing SQLite native online backup mechanism".into(),
            tags: vec!["backup".into(), "sqlite".into()],
            created_at: "2026-10-31T12:00:00Z".into(),
            updated_at: "2026-10-31T12:00:00Z".into(),
        };
        live_db
            .save_memory_note(&note)
            .expect("failed to save memory note");

        // 2. Perform live non-blocking online backup
        live_db
            .backup_online(&backup_db_path)
            .expect("online backup failed");
        assert!(backup_db_path.exists());

        // 3. Open the backup database independently and verify contents
        let restored_db = DatabaseEngine::open(&backup_db_path).expect("failed to open backup db");
        let found_symbols = restored_db
            .search_symbols_fts("chacha20_poly1305_encrypt", 10)
            .expect("symbol search failed in backup");
        assert_eq!(found_symbols.len(), 1);
        assert_eq!(found_symbols[0].name, "chacha20_poly1305_encrypt");

        let restored_notes = restored_db
            .get_memory_notes()
            .expect("notes failed in backup");
        assert_eq!(restored_notes.len(), 1);
        assert_eq!(restored_notes[0].title, "Backup Verification");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
