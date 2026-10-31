use application_core::Result;
use filesystem_engine::WorkspaceCapability;
use std::path::Path;
use symbol_index::SymbolIndex;
use syntax_engine::{SupportedLanguage, SyntaxEngine};
use tokio::sync::mpsc;
use tracing::info;

pub struct IndexProgress {
    pub files_scanned: usize,
    pub total_symbols: usize,
    pub status: String,
}

pub struct ProjectIndexer {
    index: SymbolIndex,
}

impl ProjectIndexer {
    pub fn new(index: SymbolIndex) -> Self {
        Self { index }
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
        let rel_files = cap.scan_workspace_relative_files(exclude_patterns)?;
        let mut total_symbols = 0;
        let mut scanned = 0;

        for rel_file in &rel_files {
            let ext = rel_file.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lang = SupportedLanguage::from_extension(ext);
            if lang != SupportedLanguage::Generic {
                if let Ok(content) = cap.read_file_to_string(rel_file) {
                    let symbols = SyntaxEngine::extract_symbols(&content, lang);
                    if !symbols.is_empty() {
                        let rel_path = rel_file.to_string_lossy();
                        if let Ok(count) = self.index.insert_file_symbols(&rel_path, &symbols) {
                            total_symbols += count;
                        }
                    }
                }
            }
            scanned += 1;

            if let Some(tx) = &progress_tx {
                if scanned % 20 == 0 || scanned == rel_files.len() {
                    let _ = tx
                        .send(IndexProgress {
                            files_scanned: scanned,
                            total_symbols,
                            status: format!("Indexed {}/{} files", scanned, rel_files.len()),
                        })
                        .await;
                }
            }
        }

        info!(
            "Finished indexing: {} files, {} symbols",
            scanned, total_symbols
        );
        Ok(IndexProgress {
            files_scanned: scanned,
            total_symbols,
            status: "Complete".to_string(),
        })
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
}
