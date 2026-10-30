use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentState {
    pub uri: String,
    pub language_id: String,
    pub version: i64,
    pub content: String,
}

impl DocumentState {
    pub fn new(
        uri: impl Into<String>,
        language_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            language_id: language_id.into(),
            version: 1,
            content: content.into(),
        }
    }

    pub fn to_did_open_params(&self) -> Value {
        json!({
            "textDocument": {
                "uri": self.uri,
                "languageId": self.language_id,
                "version": self.version,
                "text": self.content
            }
        })
    }

    pub fn to_did_change_params(&self) -> Value {
        json!({
            "textDocument": {
                "uri": self.uri,
                "version": self.version
            },
            "contentChanges": [
                {
                    "text": self.content
                }
            ]
        })
    }

    pub fn to_did_save_params(&self) -> Value {
        json!({
            "textDocument": {
                "uri": self.uri
            },
            "text": self.content
        })
    }

    pub fn to_did_close_params(&self) -> Value {
        json!({
            "textDocument": {
                "uri": self.uri
            }
        })
    }
}

/// Convert local file path to LSP file URI according to RFC 8089.
pub fn path_to_uri(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy().replace('\\', "/");
    let trimmed = path_str.trim_start_matches("//?/"); // strip Windows extended length prefix if present

    if trimmed.starts_with('/') {
        format!("file://{}", trimmed)
    } else {
        format!("file:///{}", trimmed)
    }
}

/// Convert LSP file URI back to local PathBuf.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    if !uri.starts_with("file://") {
        return None;
    }
    let rest = &uri["file://".len()..];
    #[cfg(windows)]
    {
        let rest = rest.trim_start_matches('/');
        Some(PathBuf::from(rest.replace('/', "\\")))
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(rest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_uri_conversion() {
        let p = PathBuf::from("C:\\projects\\aether\\src\\main.rs");
        let uri = path_to_uri(&p);
        assert!(uri.starts_with("file:///"));
        assert!(uri.contains("projects/aether/src/main.rs") || uri.contains("main.rs"));

        let converted = uri_to_path(&uri);
        assert!(converted.is_some());
    }

    #[test]
    fn test_did_open_params() {
        let doc = DocumentState::new("file:///test.rs", "rust", "fn main() {}");
        let params = doc.to_did_open_params();
        assert_eq!(params["textDocument"]["languageId"], "rust");
        assert_eq!(params["textDocument"]["version"], 1);
        assert_eq!(params["textDocument"]["text"], "fn main() {}");
    }
}
