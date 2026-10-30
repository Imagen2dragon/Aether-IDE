use application_core::Result;
use filesystem_engine::FilesystemEngine;
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

        let files = FilesystemEngine::scan_workspace_files(root_path, exclude_patterns)?;
        let mut total_symbols = 0;
        let mut scanned = 0;

        for file in &files {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lang = SupportedLanguage::from_extension(ext);
            if lang != SupportedLanguage::Generic {
                if let Ok(content) = FilesystemEngine::read_file_string(file) {
                    let symbols = SyntaxEngine::extract_symbols(&content, lang);
                    if !symbols.is_empty() {
                        let rel_path = file
                            .strip_prefix(root_path)
                            .unwrap_or(file)
                            .to_string_lossy();
                        if let Ok(count) = self.index.insert_file_symbols(&rel_path, &symbols) {
                            total_symbols += count;
                        }
                    }
                }
            }
            scanned += 1;

            if let Some(tx) = &progress_tx {
                if scanned % 20 == 0 || scanned == files.len() {
                    let _ = tx
                        .send(IndexProgress {
                            files_scanned: scanned,
                            total_symbols,
                            status: format!("Indexed {}/{} files", scanned, files.len()),
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
