use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: String,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: String,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Cpp,
    C,
    Generic,
}

impl SupportedLanguage {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "rs" => Self::Rust,
            "ts" | "tsx" => Self::TypeScript,
            "js" | "jsx" => Self::JavaScript,
            "py" => Self::Python,
            "go" => Self::Go,
            "cpp" | "cc" | "cxx" | "hpp" | "h" => Self::Cpp,
            "c" => Self::C,
            _ => Self::Generic,
        }
    }
}

pub struct SyntaxEngine;

static RUST_FN_RE: OnceLock<Regex> = OnceLock::new();
static RUST_STRUCT_RE: OnceLock<Regex> = OnceLock::new();
static TS_FN_RE: OnceLock<Regex> = OnceLock::new();
static TS_CLASS_RE: OnceLock<Regex> = OnceLock::new();
static PY_DEF_RE: OnceLock<Regex> = OnceLock::new();
static PY_CLASS_RE: OnceLock<Regex> = OnceLock::new();

impl SyntaxEngine {
    pub fn extract_symbols(source: &str, lang: SupportedLanguage) -> Vec<ExtractedSymbol> {
        let lines: Vec<&str> = source.lines().collect();
        let mut symbols = Vec::new();

        match lang {
            SupportedLanguage::Rust => {
                let fn_re = RUST_FN_RE.get_or_init(|| {
                    Regex::new(r"^\s*(?:pub(?:\([a-z\s_]+\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*(?:<[^>]+>)?\s*\((.*?)\)").unwrap()
                });
                let struct_re = RUST_STRUCT_RE.get_or_init(|| {
                    Regex::new(r"^\s*(?:pub(?:\([a-z\s_]+\))?\s+)?(?:struct|enum|trait)\s+([a-zA-Z0-9_]+)").unwrap()
                });

                for (idx, line) in lines.iter().enumerate() {
                    let line_no = idx + 1;
                    if let Some(caps) = fn_re.captures(line) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(ExtractedSymbol {
                            name: name.to_string(),
                            kind: "function".to_string(),
                            line_start: line_no,
                            line_end: line_no,
                            signature: line.trim().to_string(),
                            docstring: Self::extract_preceding_doc(&lines, idx),
                        });
                    } else if let Some(caps) = struct_re.captures(line) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(ExtractedSymbol {
                            name: name.to_string(),
                            kind: "type".to_string(),
                            line_start: line_no,
                            line_end: line_no,
                            signature: line.trim().to_string(),
                            docstring: Self::extract_preceding_doc(&lines, idx),
                        });
                    }
                }
            }
            SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
                let fn_re = TS_FN_RE.get_or_init(|| {
                    Regex::new(r"^\s*(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_]+)\s*\((.*?)\)").unwrap()
                });
                let class_re = TS_CLASS_RE.get_or_init(|| {
                    Regex::new(r"^\s*(?:export\s+)?(?:class|interface|type)\s+([a-zA-Z0-9_]+)").unwrap()
                });

                for (idx, line) in lines.iter().enumerate() {
                    let line_no = idx + 1;
                    if let Some(caps) = fn_re.captures(line) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(ExtractedSymbol {
                            name: name.to_string(),
                            kind: "function".to_string(),
                            line_start: line_no,
                            line_end: line_no,
                            signature: line.trim().to_string(),
                            docstring: Self::extract_preceding_doc(&lines, idx),
                        });
                    } else if let Some(caps) = class_re.captures(line) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(ExtractedSymbol {
                            name: name.to_string(),
                            kind: "class".to_string(),
                            line_start: line_no,
                            line_end: line_no,
                            signature: line.trim().to_string(),
                            docstring: Self::extract_preceding_doc(&lines, idx),
                        });
                    }
                }
            }
            SupportedLanguage::Python => {
                let def_re = PY_DEF_RE.get_or_init(|| {
                    Regex::new(r"^\s*(?:async\s+)?def\s+([a-zA-Z0-9_]+)\s*\((.*?)\):").unwrap()
                });
                let class_re = PY_CLASS_RE.get_or_init(|| {
                    Regex::new(r"^\s*class\s+([a-zA-Z0-9_]+)(?:\(.*?\))?:").unwrap()
                });

                for (idx, line) in lines.iter().enumerate() {
                    let line_no = idx + 1;
                    if let Some(caps) = def_re.captures(line) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(ExtractedSymbol {
                            name: name.to_string(),
                            kind: "function".to_string(),
                            line_start: line_no,
                            line_end: line_no,
                            signature: line.trim().to_string(),
                            docstring: None,
                        });
                    } else if let Some(caps) = class_re.captures(line) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(ExtractedSymbol {
                            name: name.to_string(),
                            kind: "class".to_string(),
                            line_start: line_no,
                            line_end: line_no,
                            signature: line.trim().to_string(),
                            docstring: None,
                        });
                    }
                }
            }
            _ => {}
        }

        symbols
    }

    fn extract_preceding_doc(lines: &[&str], current_idx: usize) -> Option<String> {
        if current_idx == 0 {
            return None;
        }
        let mut doc_lines = Vec::new();
        let mut idx = current_idx;
        while idx > 0 {
            idx -= 1;
            let line = lines[idx].trim();
            if line.starts_with("///") || line.starts_with("//!") || line.starts_with('*') || line.starts_with("/**") {
                doc_lines.push(line);
            } else {
                break;
            }
        }
        if doc_lines.is_empty() {
            None
        } else {
            doc_lines.reverse();
            Some(doc_lines.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_symbol_extraction() {
        let code = r#"
/// Computes the sum of two integers
pub fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

pub struct UserProfile {
    pub username: String,
}
"#;
        let symbols = SyntaxEngine::extract_symbols(code, SupportedLanguage::Rust);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "add_numbers");
        assert_eq!(symbols[0].kind, "function");
        assert!(symbols[0].docstring.as_ref().unwrap().contains("Computes the sum"));
        assert_eq!(symbols[1].name, "UserProfile");
        assert_eq!(symbols[1].kind, "type");
    }
}
