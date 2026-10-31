use application_core::{AetherError, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

/// Wraps a Yrs CRDT document managing collaborative text state.
#[derive(Clone)]
pub struct CrdtDocument {
    id: String,
    doc: Arc<Doc>,
}

impl CrdtDocument {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            doc: Arc::new(Doc::new()),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Read the full text content from the default "content" collection.
    pub fn get_text(&self) -> String {
        let text = self.doc.get_or_insert_text("content");
        let txn = self.doc.transact();
        text.get_string(&txn)
    }

    /// Set or replace the entire text content.
    pub fn set_text(&self, content: &str) {
        let text = self.doc.get_or_insert_text("content");
        let mut txn = self.doc.transact_mut();
        let len = text.get_string(&txn).chars().count() as u32;
        if len > 0 {
            text.remove_range(&mut txn, 0, len);
        }
        text.push(&mut txn, content);
    }

    /// Insert text at a specific character index.
    pub fn insert_text(&self, index: u32, chunk: &str) {
        let text = self.doc.get_or_insert_text("content");
        let mut txn = self.doc.transact_mut();
        text.insert(&mut txn, index, chunk);
    }

    /// Delete a character range starting at index.
    pub fn delete_text(&self, index: u32, len: u32) {
        let text = self.doc.get_or_insert_text("content");
        let mut txn = self.doc.transact_mut();
        text.remove_range(&mut txn, index, len);
    }

    /// Computes the current state vector encoded in Yjs V1 wire format (base64).
    pub fn get_state_vector_base64(&self) -> String {
        let txn = self.doc.transact();
        let sv = txn.state_vector();
        let bytes = sv.encode_v1();
        BASE64.encode(bytes)
    }

    /// Produces a full document state update encoded in Yjs V1 wire format (base64).
    pub fn get_full_update_base64(&self) -> String {
        let txn = self.doc.transact();
        let bytes = txn.encode_state_as_update_v1(&StateVector::default());
        BASE64.encode(bytes)
    }

    /// Produces a delta update relative to a remote peer's state vector (base64).
    pub fn get_diff_base64(&self, remote_sv_base64: &str) -> Result<String> {
        let sv_bytes = BASE64
            .decode(remote_sv_base64)
            .map_err(|e| AetherError::Internal(format!("Invalid state vector base64: {e}")))?;
        let remote_sv = StateVector::decode_v1(&sv_bytes)
            .map_err(|e| AetherError::Internal(format!("Corrupt state vector payload: {e}")))?;
        let txn = self.doc.transact();
        let diff_bytes = txn.encode_diff_v1(&remote_sv);
        Ok(BASE64.encode(diff_bytes))
    }

    /// Applies a remote binary update encoded in Yjs V1 format (base64).
    pub fn apply_update_base64(&self, update_base64: &str) -> Result<()> {
        let update_bytes = BASE64
            .decode(update_base64)
            .map_err(|e| AetherError::Internal(format!("Invalid update base64: {e}")))?;
        let update = Update::decode_v1(&update_bytes)
            .map_err(|e| AetherError::Internal(format!("Corrupt update payload: {e}")))?;
        let mut txn = self.doc.transact_mut();
        txn.apply_update(update)
            .map_err(|e| AetherError::Internal(format!("Failed to apply CRDT update: {e}")))?;
        Ok(())
    }
}

/// Global registry of active CRDT documents in the local workspace.
#[derive(Default)]
pub struct CrdtManager {
    documents: RwLock<HashMap<String, CrdtDocument>>,
}

impl CrdtManager {
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_or_create(&self, doc_id: &str) -> CrdtDocument {
        let mut docs = self.documents.write();
        docs.entry(doc_id.to_string())
            .or_insert_with(|| CrdtDocument::new(doc_id))
            .clone()
    }

    pub fn get(&self, doc_id: &str) -> Option<CrdtDocument> {
        self.documents.read().get(doc_id).cloned()
    }

    pub fn remove(&self, doc_id: &str) -> Option<CrdtDocument> {
        self.documents.write().remove(doc_id)
    }

    pub fn list_ids(&self) -> Vec<String> {
        self.documents.read().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crdt_basic_editing() {
        let doc = CrdtDocument::new("test-doc-1");
        assert_eq!(doc.get_text(), "");

        doc.set_text("Hello, World!");
        assert_eq!(doc.get_text(), "Hello, World!");

        doc.insert_text(7, "Aether ");
        assert_eq!(doc.get_text(), "Hello, Aether World!");

        doc.delete_text(0, 7);
        assert_eq!(doc.get_text(), "Aether World!");
    }

    #[test]
    fn test_crdt_two_peer_convergence() {
        let peer_a = CrdtDocument::new("shared-doc");
        let peer_b = CrdtDocument::new("shared-doc");

        peer_a.set_text("Rust Systems");
        let update_from_a = peer_a.get_full_update_base64();
        peer_b.apply_update_base64(&update_from_a).unwrap();
        assert_eq!(peer_b.get_text(), "Rust Systems");

        // Concurrent edits on both peers
        peer_a.insert_text(0, "High-Performance ");
        peer_b.insert_text(12, " Architecture");

        let diff_a = peer_a
            .get_diff_base64(&peer_b.get_state_vector_base64())
            .unwrap();
        let diff_b = peer_b
            .get_diff_base64(&peer_a.get_state_vector_base64())
            .unwrap();

        peer_b.apply_update_base64(&diff_a).unwrap();
        peer_a.apply_update_base64(&diff_b).unwrap();

        // Both peers must converge to identical deterministic state
        assert_eq!(peer_a.get_text(), peer_b.get_text());
        assert!(peer_a.get_text().contains("High-Performance"));
        assert!(peer_a.get_text().contains("Rust Systems"));
        assert!(peer_a.get_text().contains("Architecture"));
    }
}
