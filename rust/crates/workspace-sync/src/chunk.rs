use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024; // 64 KB

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChunk {
    pub path: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: Vec<u8>,
    pub sha256: String,
}

pub fn slice_file(path: &str, content: &[u8], chunk_size: usize) -> Vec<FileChunk> {
    if content.is_empty() {
        let hasher = Sha256::new();
        let sha256 = format!("{:x}", hasher.finalize());
        return vec![FileChunk {
            path: path.to_string(),
            chunk_index: 0,
            total_chunks: 1,
            data: Vec::new(),
            sha256,
        }];
    }

    let chunks: Vec<&[u8]> = content.chunks(chunk_size).collect();
    let total_chunks = chunks.len() as u32;

    chunks
        .into_iter()
        .enumerate()
        .map(|(i, slice)| {
            let mut hasher = Sha256::new();
            hasher.update(slice);
            let sha256 = format!("{:x}", hasher.finalize());

            FileChunk {
                path: path.to_string(),
                chunk_index: i as u32,
                total_chunks,
                data: slice.to_vec(),
                sha256,
            }
        })
        .collect()
}

pub fn reassemble_chunks(mut chunks: Vec<FileChunk>) -> Result<Vec<u8>, String> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    chunks.sort_by_key(|c| c.chunk_index);
    let total = chunks[0].total_chunks as usize;

    if chunks.len() != total {
        return Err(format!(
            "Missing chunks: expected {}, got {}",
            total,
            chunks.len()
        ));
    }

    let mut output = Vec::new();
    for (expected_idx, chunk) in chunks.into_iter().enumerate() {
        if chunk.chunk_index as usize != expected_idx {
            return Err(format!(
                "Out of order chunk: expected index {}, got {}",
                expected_idx, chunk.chunk_index
            ));
        }

        // Verify chunk hash integrity
        let mut hasher = Sha256::new();
        hasher.update(&chunk.data);
        let actual_hash = format!("{:x}", hasher.finalize());
        if actual_hash != chunk.sha256 {
            return Err(format!(
                "Corrupt chunk at index {}: hash mismatch",
                chunk.chunk_index
            ));
        }

        output.extend_from_slice(&chunk.data);
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_and_reassemble_roundtrip() {
        let data = b"AetherIDE High-Speed Incremental File Synchronization Test Payload. Repeated for length. ".repeat(1000);
        let chunks = slice_file("src/large_file.rs", &data, 1024);

        assert!(chunks.len() > 1);
        let reassembled = reassemble_chunks(chunks).unwrap();
        assert_eq!(reassembled, data);
    }

    #[test]
    fn test_rejects_corrupted_chunk() {
        let data = b"Hello world".repeat(100);
        let mut chunks = slice_file("test.txt", &data, 64);
        chunks[0].data[0] ^= 0xFF; // corrupt

        let res = reassemble_chunks(chunks);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("hash mismatch"));
    }
}
