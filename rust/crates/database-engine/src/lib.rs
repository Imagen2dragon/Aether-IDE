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

impl DatabaseEngine {
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AetherError::Database(e.to_string()))?;
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
        let conn = Connection::open(path)
            .map_err(|e| AetherError::Database(e.to_string()))?;

        // Enable WAL mode, memory cache, and synchronous=NORMAL
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA cache_size = -64000;",
        ).map_err(|e| AetherError::Database(e.to_string()))?;

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
            );",
        ).map_err(|e| AetherError::Database(e.to_string()))?;

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

        let rows = stmt.query_map(params![formatted_query, limit as i64], |row| {
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
        }).map_err(|e| AetherError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(sym) = r {
                results.push(sym);
            }
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

        let rows = stmt.query_map(params![task_id], |row| {
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
        }).map_err(|e| AetherError::Database(e.to_string()))?;

        let mut results = Vec::new();
        for r in rows {
            if let Ok(rec) = r {
                results.push(rec);
            }
        }
        Ok(results)
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

        let search_results = db.search_symbols_fts("validate_jwt", 10).expect("fts failed");
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
}
