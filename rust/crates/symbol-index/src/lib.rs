use application_core::Result;
use database_engine::{DatabaseEngine, SymbolRecord};
use syntax_engine::ExtractedSymbol;

pub struct SymbolIndex {
    db: DatabaseEngine,
}

impl SymbolIndex {
    pub fn new(db: DatabaseEngine) -> Self {
        Self { db }
    }

    pub fn insert_file_symbols(
        &self,
        file_path: &str,
        symbols: &[ExtractedSymbol],
    ) -> Result<usize> {
        let mut count = 0;
        for sym in symbols {
            let record = SymbolRecord {
                id: None,
                file_path: file_path.to_string(),
                name: sym.name.clone(),
                kind: sym.kind.clone(),
                line_start: sym.line_start,
                line_end: sym.line_end,
                signature: sym.signature.clone(),
                docstring: sym.docstring.clone(),
            };
            self.db.insert_symbol(&record)?;
            count += 1;
        }
        Ok(count)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SymbolRecord>> {
        self.db.search_symbols_fts(query, limit)
    }

    pub fn remove_file_symbols(&self, file_path: &str) -> Result<usize> {
        self.db.delete_file_symbols(file_path)
    }
}
