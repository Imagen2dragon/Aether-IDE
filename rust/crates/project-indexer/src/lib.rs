use application_core::Result;
use filesystem_engine::WorkspaceCapability;
use std::path::Path;
use symbol_index::SymbolIndex;
use syntax_engine::{SupportedLanguage, SyntaxEngine};
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IndexingStatus {
    Complete,
    CompleteWithWarnings,
    Partial,
    Cancelled,
    Failed,
}

impl std::fmt::Display for IndexingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Complete => write!(f, "Complete"),
            Self::CompleteWithWarnings => write!(f, "CompleteWithWarnings"),
            Self::Partial => write!(f, "Partial"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexProgress {
    pub files_scanned: usize,
    pub files_discovered: usize,
    pub files_indexed: usize,
    pub files_failed: usize,
    pub total_symbols: usize,
    pub status: String,
    pub indexing_status: IndexingStatus,
    pub warnings: Vec<String>,
}

pub struct ProjectIndexer {
    index: SymbolIndex,
}

impl ProjectIndexer {
    pub fn new(index: SymbolIndex) -> Self {
        Self { index }
    }

    pub fn index(&self) -> &SymbolIndex {
        &self.index
    }

    pub async fn index_workspace<P: AsRef<Path>>(
        &self,
        root: P,
        exclude_patterns: &[String],
        progress_tx: Option<mpsc::Sender<IndexProgress>>,
    ) -> Result<IndexProgress> {
        let root_path = root.as_ref();
        info!("Beginning indexing workspace at {:?}", root_path);

        // SEC-CAP-01: Anchor indexing strictly to WorkspaceCapability
        let cap = WorkspaceCapability::open(root_path)?;
        let scan_report = cap.scan_workspace_detailed(exclude_patterns)?;
        let rel_files = scan_report.discovered_files;
        let files_discovered = rel_files.len();

        let mut warnings = Vec::new();
        for perm_err in &scan_report.permission_errors {
            warnings.push(format!(
                "Permission denied reading directory: {:?}",
                perm_err
            ));
        }
        for trans_err in &scan_report.transient_errors {
            warnings.push(format!(
                "Transient error reading directory or file: {:?}",
                trans_err
            ));
        }

        let mut total_symbols = 0;
        let mut scanned = 0;
        let mut files_indexed = 0;
        let mut files_failed = 0;

        for rel_file in &rel_files {
            let ext = rel_file.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lang = SupportedLanguage::from_extension(ext);
            if lang != SupportedLanguage::Generic {
                match cap.read_file_to_string(rel_file) {
                    Ok(content) => {
                        let symbols = SyntaxEngine::extract_symbols(&content, lang);
                        if !symbols.is_empty() {
                            let rel_path = rel_file.to_string_lossy();
                            match self.index.insert_file_symbols(&rel_path, &symbols) {
                                Ok(count) => {
                                    total_symbols += count;
                                    files_indexed += 1;
                                }
                                Err(e) => {
                                    files_failed += 1;
                                    warnings.push(format!(
                                        "Failed to index symbols for '{}': {}",
                                        rel_file.display(),
                                        e
                                    ));
                                }
                            }
                        } else {
                            files_indexed += 1;
                        }
                    }
                    Err(e) => {
                        files_failed += 1;
                        warnings.push(format!(
                            "Failed to read file '{}': {}",
                            rel_file.display(),
                            e
                        ));
                    }
                }
            } else {
                files_indexed += 1;
            }
            scanned += 1;

            if let Some(tx) = &progress_tx {
                if scanned % 20 == 0 || scanned == rel_files.len() {
                    let _ = tx
                        .send(IndexProgress {
                            files_scanned: scanned,
                            files_discovered,
                            files_indexed,
                            files_failed,
                            total_symbols,
                            status: format!("Indexed {}/{} files", scanned, rel_files.len()),
                            indexing_status: IndexingStatus::Partial,
                            warnings: warnings.clone(),
                        })
                        .await;
                }
            }
        }

        let final_status = if files_discovered == 0 {
            IndexingStatus::Complete
        } else if files_failed == files_discovered {
            IndexingStatus::Failed
        } else if files_failed > 0 && (files_failed as f32 / files_discovered as f32) > 0.25 {
            IndexingStatus::Partial
        } else if !warnings.is_empty() || files_failed > 0 || !scan_report.is_complete {
            IndexingStatus::CompleteWithWarnings
        } else {
            IndexingStatus::Complete
        };

        info!(
            "Finished indexing: {} discovered, {} indexed, {} failed, {} symbols, status: {}",
            files_discovered, files_indexed, files_failed, total_symbols, final_status
        );

        Ok(IndexProgress {
            files_scanned: scanned,
            files_discovered,
            files_indexed,
            files_failed,
            total_symbols,
            status: final_status.to_string(),
            indexing_status: final_status,
            warnings,
        })
    }

    /// Indexes a single file incrementally without triggering a full workspace scan.
    pub fn index_file_incremental<P: AsRef<Path>>(
        &self,
        root: P,
        rel_file: &Path,
    ) -> Result<usize> {
        let cap = WorkspaceCapability::open(root)?;
        let ext = rel_file.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = SupportedLanguage::from_extension(ext);
        if lang == SupportedLanguage::Generic {
            return Ok(0);
        }

        let content = cap.read_file_to_string(rel_file)?;
        let symbols = SyntaxEngine::extract_symbols(&content, lang);
        let rel_path = rel_file.to_string_lossy();
        self.index.insert_file_symbols(&rel_path, &symbols)
    }

    /// Removes symbol index records for a deleted or moved file.
    pub fn remove_file_symbols(&self, rel_file: &Path) -> Result<()> {
        let rel_path = rel_file.to_string_lossy();
        self.index.remove_file_symbols(&rel_path).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database_engine::DatabaseEngine;
    use std::fs;
    use symbol_index::SymbolIndex;

    #[tokio::test]
    async fn test_adversarial_indexer_sentinel_isolation() {
        let ws = std::env::temp_dir().join(format!(
            "aether_indexer_ws_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let outside_dir = std::env::temp_dir().join(format!(
            "aether_indexer_outside_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&ws).unwrap();
        fs::create_dir_all(&outside_dir).unwrap();

        // Confidential outside sentinel file
        let outside_file = outside_dir.join("outside_secret.rs");
        fs::write(
            &outside_file,
            "pub fn confidential_sentinel_outside_key() -> u64 { 0xdeadbeef }\npub struct OutsideSecretVault;\n",
        )
        .unwrap();

        // Valid inside workspace file
        let src_dir = ws.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let legit_file = src_dir.join("legit.rs");
        fs::write(
            &legit_file,
            "pub fn legitimate_workspace_function() -> bool { true }\n",
        )
        .unwrap();

        // Adversary establishes symlink / junction inside workspace pointing to outside confidential directory
        let symlink_path = ws.join("adversarial_symlink");
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("cmd")
                .args([
                    "/C",
                    "mklink",
                    "/J",
                    &symlink_path.to_string_lossy(),
                    &outside_dir.to_string_lossy(),
                ])
                .output();
        }
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink(&outside_dir, &symlink_path);
        }

        let db = DatabaseEngine::open_in_memory().unwrap();
        let symbol_index = SymbolIndex::new(db.clone());
        let indexer = ProjectIndexer::new(symbol_index);

        let progress = indexer
            .index_workspace(&ws, &[], None)
            .await
            .expect("Indexing must succeed");

        assert!(progress.files_scanned >= 1);

        // 1. Verify outside sentinel symbols were NEVER ingested
        let leaked_fn = db
            .search_symbols_fts("confidential_sentinel_outside_key", 10)
            .unwrap();
        assert!(
            leaked_fn.is_empty(),
            "Adversarial symlink caused outside confidential function to leak into symbol index!"
        );

        let leaked_struct = db.search_symbols_fts("OutsideSecretVault", 10).unwrap();
        assert!(
            leaked_struct.is_empty(),
            "Adversarial symlink caused outside confidential struct to leak into symbol index!"
        );

        // 2. Verify legitimate workspace symbols ARE indexed
        let legit_symbols = db
            .search_symbols_fts("legitimate_workspace_function", 10)
            .unwrap();
        assert!(
            !legit_symbols.is_empty(),
            "Legitimate workspace function must be indexed"
        );
        assert_eq!(legit_symbols[0].name, "legitimate_workspace_function");

        #[cfg(windows)]
        let _ = fs::remove_dir(&symlink_path);
        let _ = fs::remove_dir_all(&ws);
        let _ = fs::remove_dir_all(&outside_dir);
    }

    #[tokio::test]
    async fn test_incremental_indexing_and_removal() {
        let ws = std::env::temp_dir().join(format!(
            "aether_indexer_inc_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(ws.join("src")).unwrap();

        let file_path = ws.join("src/service.rs");
        fs::write(&file_path, "pub fn execute_operation() -> bool { true }\n").unwrap();

        let db = DatabaseEngine::open_in_memory().unwrap();
        let symbol_index = SymbolIndex::new(db.clone());
        let indexer = ProjectIndexer::new(symbol_index);

        let initial_progress = indexer
            .index_workspace(&ws, &[], None)
            .await
            .expect("Workspace indexing must succeed");
        assert_eq!(initial_progress.indexing_status, IndexingStatus::Complete);
        assert_eq!(initial_progress.files_discovered, 1);
        assert_eq!(initial_progress.files_indexed, 1);
        assert_eq!(initial_progress.files_failed, 0);

        // Verify symbol exists
        let found = db.search_symbols_fts("execute_operation", 5).unwrap();
        assert_eq!(found.len(), 1);

        // Incremental add of a new file
        let file2 = ws.join("src/handler.rs");
        fs::write(&file2, "pub fn handle_request() -> u32 { 42 }\n").unwrap();
        let count = indexer
            .index_file_incremental(&ws, Path::new("src/handler.rs"))
            .expect("Incremental indexing must succeed");
        assert_eq!(count, 1);

        let found2 = db.search_symbols_fts("handle_request", 5).unwrap();
        assert_eq!(found2.len(), 1);

        // Incremental removal
        indexer
            .remove_file_symbols(Path::new("src/handler.rs"))
            .expect("Removal must succeed");
        let after_removal = db.search_symbols_fts("handle_request", 5).unwrap();
        assert_eq!(after_removal.len(), 0);

        let _ = fs::remove_dir_all(ws);
    }
}
