use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use syn::spanned::Spanned;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

static TS_FN_RE: OnceLock<Regex> = OnceLock::new();
static TS_CLASS_RE: OnceLock<Regex> = OnceLock::new();
static TS_INTERFACE_RE: OnceLock<Regex> = OnceLock::new();
static TS_TYPE_RE: OnceLock<Regex> = OnceLock::new();
static TS_ARROW_RE: OnceLock<Regex> = OnceLock::new();
static PY_DEF_RE: OnceLock<Regex> = OnceLock::new();
static PY_CLASS_RE: OnceLock<Regex> = OnceLock::new();

impl SyntaxEngine {
    pub fn extract_symbols(source: &str, lang: SupportedLanguage) -> Vec<ExtractedSymbol> {
        match lang {
            SupportedLanguage::Rust => Self::extract_rust_ast(source),
            SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
                Self::extract_ts_js(source)
            }
            SupportedLanguage::Python => Self::extract_python(source),
            _ => Self::extract_generic(source),
        }
    }

    // -------------------------------------------------------------------------
    // Rust: Real AST Parsing using syn
    // -------------------------------------------------------------------------
    fn extract_rust_ast(source: &str) -> Vec<ExtractedSymbol> {
        let file = match syn::parse_file(source) {
            Ok(f) => f,
            Err(_) => return Self::extract_rust_fallback(source),
        };

        let mut symbols = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for item in &file.items {
            Self::visit_rust_item(item, &lines, &mut symbols);
        }

        symbols
    }

    fn visit_rust_item(item: &syn::Item, lines: &[&str], symbols: &mut Vec<ExtractedSymbol>) {
        match item {
            syn::Item::Fn(f) => {
                let name = f.sig.ident.to_string();
                let span = f.span();
                let line_start = span.start().line;
                let line_end = span.end().line;
                let docstring = Self::extract_syn_doc(&f.attrs);
                let sig = Self::slice_signature(lines, line_start);

                symbols.push(ExtractedSymbol {
                    name,
                    kind: "function".into(),
                    line_start,
                    line_end,
                    signature: sig,
                    docstring,
                });
            }
            syn::Item::Struct(s) => {
                let name = s.ident.to_string();
                let span = s.span();
                let line_start = span.start().line;
                let line_end = span.end().line;
                let docstring = Self::extract_syn_doc(&s.attrs);
                let sig = Self::slice_signature(lines, line_start);

                symbols.push(ExtractedSymbol {
                    name,
                    kind: "struct".into(),
                    line_start,
                    line_end,
                    signature: sig,
                    docstring,
                });
            }
            syn::Item::Enum(e) => {
                let name = e.ident.to_string();
                let span = e.span();
                let line_start = span.start().line;
                let line_end = span.end().line;
                let docstring = Self::extract_syn_doc(&e.attrs);
                let sig = Self::slice_signature(lines, line_start);

                symbols.push(ExtractedSymbol {
                    name,
                    kind: "enum".into(),
                    line_start,
                    line_end,
                    signature: sig,
                    docstring,
                });
            }
            syn::Item::Trait(t) => {
                let name = t.ident.to_string();
                let span = t.span();
                let line_start = span.start().line;
                let line_end = span.end().line;
                let docstring = Self::extract_syn_doc(&t.attrs);
                let sig = Self::slice_signature(lines, line_start);

                symbols.push(ExtractedSymbol {
                    name,
                    kind: "trait".into(),
                    line_start,
                    line_end,
                    signature: sig,
                    docstring,
                });
            }
            syn::Item::Type(t) => {
                let name = t.ident.to_string();
                let span = t.span();
                let line_start = span.start().line;
                let line_end = span.end().line;
                let docstring = Self::extract_syn_doc(&t.attrs);
                let sig = Self::slice_signature(lines, line_start);

                symbols.push(ExtractedSymbol {
                    name,
                    kind: "type".into(),
                    line_start,
                    line_end,
                    signature: sig,
                    docstring,
                });
            }
            syn::Item::Impl(i) => {
                for impl_item in &i.items {
                    if let syn::ImplItem::Fn(m) = impl_item {
                        let name = m.sig.ident.to_string();
                        let span = m.span();
                        let line_start = span.start().line;
                        let line_end = span.end().line;
                        let docstring = Self::extract_syn_doc(&m.attrs);
                        let sig = Self::slice_signature(lines, line_start);

                        symbols.push(ExtractedSymbol {
                            name,
                            kind: "method".into(),
                            line_start,
                            line_end,
                            signature: sig,
                            docstring,
                        });
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    for sub_item in items {
                        Self::visit_rust_item(sub_item, lines, symbols);
                    }
                }
            }
            _ => {}
        }
    }

    fn extract_syn_doc(attrs: &[syn::Attribute]) -> Option<String> {
        let mut docs = Vec::new();
        for attr in attrs {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(expr_lit) = &nv.value {
                        if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                            docs.push(lit_str.value().trim().to_string());
                        }
                    }
                }
            }
        }
        if docs.is_empty() {
            None
        } else {
            Some(docs.join("\n"))
        }
    }

    fn slice_signature(lines: &[&str], line_start: usize) -> String {
        if line_start == 0 || line_start > lines.len() {
            return "".into();
        }
        lines[line_start - 1].trim().to_string()
    }

    fn extract_rust_fallback(source: &str) -> Vec<ExtractedSymbol> {
        let sanitized = Self::strip_comments_and_strings(source, false);
        let mut symbols = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        let fn_re =
            Regex::new(r"(?:pub(?:\([a-z\s_]+\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)").unwrap();
        let type_re = Regex::new(
            r"(?:pub(?:\([a-z\s_]+\))?\s+)?(?:struct|enum|trait|type)\s+([a-zA-Z0-9_]+)",
        )
        .unwrap();

        for (idx, line) in sanitized.lines().enumerate() {
            let line_no = idx + 1;
            if let Some(caps) = fn_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "function".to_string(),
                    line_start: line_no,
                    line_end: line_no,
                    signature: lines.get(idx).copied().unwrap_or(line).trim().to_string(),
                    docstring: None,
                });
            } else if let Some(caps) = type_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "type".to_string(),
                    line_start: line_no,
                    line_end: line_no,
                    signature: lines.get(idx).copied().unwrap_or(line).trim().to_string(),
                    docstring: None,
                });
            }
        }

        symbols
    }

    // -------------------------------------------------------------------------
    // TypeScript / JavaScript AST Extraction (Sanitizing Comments & Literals)
    // -------------------------------------------------------------------------
    fn extract_ts_js(source: &str) -> Vec<ExtractedSymbol> {
        let (sanitized, doc_map) = Self::strip_c_style_comments_with_docs(source);
        let raw_lines: Vec<&str> = source.lines().collect();
        let mut symbols = Vec::new();

        let fn_re = TS_FN_RE.get_or_init(|| {
            Regex::new(r"(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_$]+)\s*\(").unwrap()
        });
        let class_re = TS_CLASS_RE.get_or_init(|| {
            Regex::new(r"(?:export\s+)?(?:abstract\s+)?class\s+([a-zA-Z0-9_$]+)").unwrap()
        });
        let interface_re = TS_INTERFACE_RE
            .get_or_init(|| Regex::new(r"(?:export\s+)?interface\s+([a-zA-Z0-9_$]+)").unwrap());
        let type_re = TS_TYPE_RE
            .get_or_init(|| Regex::new(r"(?:export\s+)?type\s+([a-zA-Z0-9_$]+)\s*=").unwrap());
        let arrow_re = TS_ARROW_RE.get_or_init(|| {
            Regex::new(r"(?:export\s+)?(?:const|let|var)\s+([a-zA-Z0-9_$]+)\s*=\s*(?:async\s*)?(?:\([^)]*\)|[a-zA-Z0-9_$]+)\s*=>").unwrap()
        });

        for (idx, line) in sanitized.lines().enumerate() {
            let line_no = idx + 1;
            let raw_line = raw_lines.get(idx).copied().unwrap_or("").trim().to_string();

            if let Some(caps) = fn_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "function".to_string(),
                    line_start: line_no,
                    line_end: Self::compute_brace_end(&sanitized, idx),
                    signature: raw_line,
                    docstring: doc_map.get(&line_no).cloned(),
                });
            } else if let Some(caps) = class_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "class".to_string(),
                    line_start: line_no,
                    line_end: Self::compute_brace_end(&sanitized, idx),
                    signature: raw_line,
                    docstring: doc_map.get(&line_no).cloned(),
                });
            } else if let Some(caps) = interface_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "interface".to_string(),
                    line_start: line_no,
                    line_end: Self::compute_brace_end(&sanitized, idx),
                    signature: raw_line,
                    docstring: doc_map.get(&line_no).cloned(),
                });
            } else if let Some(caps) = type_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "type".to_string(),
                    line_start: line_no,
                    line_end: line_no,
                    signature: raw_line,
                    docstring: doc_map.get(&line_no).cloned(),
                });
            } else if let Some(caps) = arrow_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "function".to_string(),
                    line_start: line_no,
                    line_end: Self::compute_brace_end(&sanitized, idx),
                    signature: raw_line,
                    docstring: doc_map.get(&line_no).cloned(),
                });
            }
        }

        symbols
    }

    // -------------------------------------------------------------------------
    // Python AST Extraction
    // -------------------------------------------------------------------------
    fn extract_python(source: &str) -> Vec<ExtractedSymbol> {
        let raw_lines: Vec<&str> = source.lines().collect();
        let sanitized = Self::strip_python_comments_and_strings(source);
        let mut symbols = Vec::new();

        let def_re = PY_DEF_RE.get_or_init(|| {
            Regex::new(r"^\s*(?:async\s+)?def\s+([a-zA-Z0-9_]+)\s*\((.*?)\)").unwrap()
        });
        let class_re =
            PY_CLASS_RE.get_or_init(|| Regex::new(r"^\s*class\s+([a-zA-Z0-9_]+)").unwrap());

        for (idx, line) in sanitized.lines().enumerate() {
            let line_no = idx + 1;
            let raw_line = raw_lines.get(idx).copied().unwrap_or("").trim().to_string();

            if let Some(caps) = def_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let doc = Self::extract_python_docstring(&raw_lines, idx);
                let line_end = Self::compute_indent_end(&raw_lines, idx);

                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "function".to_string(),
                    line_start: line_no,
                    line_end,
                    signature: raw_line,
                    docstring: doc,
                });
            } else if let Some(caps) = class_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let doc = Self::extract_python_docstring(&raw_lines, idx);
                let line_end = Self::compute_indent_end(&raw_lines, idx);

                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "class".to_string(),
                    line_start: line_no,
                    line_end,
                    signature: raw_line,
                    docstring: doc,
                });
            }
        }

        symbols
    }

    fn extract_generic(source: &str) -> Vec<ExtractedSymbol> {
        let sanitized = Self::strip_comments_and_strings(source, false);
        let lines: Vec<&str> = source.lines().collect();
        let mut symbols = Vec::new();

        let fn_re = Regex::new(r"(?:fn|func|function|def)\s+([a-zA-Z0-9_]+)").unwrap();
        for (idx, line) in sanitized.lines().enumerate() {
            let line_no = idx + 1;
            if let Some(caps) = fn_re.captures(line) {
                let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                symbols.push(ExtractedSymbol {
                    name: name.to_string(),
                    kind: "function".to_string(),
                    line_start: line_no,
                    line_end: line_no,
                    signature: lines.get(idx).copied().unwrap_or(line).trim().to_string(),
                    docstring: None,
                });
            }
        }
        symbols
    }

    // -------------------------------------------------------------------------
    // Lexical Sanitization Helpers
    // -------------------------------------------------------------------------
    fn strip_c_style_comments_with_docs(
        source: &str,
    ) -> (String, std::collections::HashMap<usize, String>) {
        let mut sanitized = String::with_capacity(source.len());
        let mut doc_map = std::collections::HashMap::new();

        let mut in_single_comment = false;
        let mut in_block_comment = false;
        let mut block_doc = String::new();
        let mut is_doc_comment = false;
        let mut pending_doc: Option<String> = None;
        let mut in_string = false;
        let mut quote_char = ' ';
        let mut line_no = 1;
        let chars: Vec<char> = source.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            let next = if i + 1 < chars.len() {
                Some(chars[i + 1])
            } else {
                None
            };

            if c == '\n' {
                line_no += 1;
                if in_single_comment {
                    in_single_comment = false;
                }
                sanitized.push('\n');
                i += 1;
                continue;
            }

            if in_single_comment {
                sanitized.push(' ');
                i += 1;
                continue;
            }

            if in_block_comment {
                if c == '*' && next == Some('/') {
                    in_block_comment = false;
                    sanitized.push(' ');
                    sanitized.push(' ');
                    if is_doc_comment && !block_doc.trim().is_empty() {
                        pending_doc = Some(block_doc.trim().to_string());
                    }
                    block_doc.clear();
                    is_doc_comment = false;
                    i += 2;
                    continue;
                } else {
                    if is_doc_comment {
                        block_doc.push(c);
                    }
                    sanitized.push(' ');
                    i += 1;
                    continue;
                }
            }

            if in_string {
                if c == '\\' {
                    sanitized.push(' ');
                    sanitized.push(' ');
                    i += 2;
                    continue;
                } else if c == quote_char {
                    in_string = false;
                    sanitized.push(' ');
                    i += 1;
                    continue;
                } else {
                    sanitized.push(' ');
                    i += 1;
                    continue;
                }
            }

            // If we have a pending doc and we encounter code, bind it to this line
            if !c.is_whitespace() && c != '/' {
                if let Some(doc) = pending_doc.take() {
                    doc_map.insert(line_no, doc);
                }
            }

            // Check start of string
            if c == '"' || c == '\'' || c == '`' {
                in_string = true;
                quote_char = c;
                sanitized.push(' ');
                i += 1;
                continue;
            }

            // Check start of single comment
            if c == '/' && next == Some('/') {
                in_single_comment = true;
                sanitized.push(' ');
                sanitized.push(' ');
                i += 2;
                continue;
            }

            // Check start of block comment
            if c == '/' && next == Some('*') {
                in_block_comment = true;
                is_doc_comment = i + 2 < chars.len() && chars[i + 2] == '*';
                sanitized.push(' ');
                sanitized.push(' ');
                i += 2;
                if is_doc_comment {
                    i += 1;
                    sanitized.push(' ');
                }
                continue;
            }

            sanitized.push(c);
            i += 1;
        }

        (sanitized, doc_map)
    }

    fn strip_python_comments_and_strings(source: &str) -> String {
        let mut sanitized = String::with_capacity(source.len());
        let mut in_string = false;
        let mut quote_char = ' ';
        let mut is_triple = false;
        let chars: Vec<char> = source.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            if in_string {
                if is_triple {
                    if i + 2 < chars.len()
                        && chars[i] == quote_char
                        && chars[i + 1] == quote_char
                        && chars[i + 2] == quote_char
                    {
                        in_string = false;
                        sanitized.push_str("   ");
                        i += 3;
                        continue;
                    }
                } else if c == quote_char && (i == 0 || chars[i - 1] != '\\') {
                    in_string = false;
                    sanitized.push(' ');
                    i += 1;
                    continue;
                }
                if c == '\n' {
                    sanitized.push('\n');
                } else {
                    sanitized.push(' ');
                }
                i += 1;
                continue;
            }

            // Check triple quotes
            if i + 2 < chars.len()
                && (c == '"' || c == '\'')
                && chars[i + 1] == c
                && chars[i + 2] == c
            {
                in_string = true;
                quote_char = c;
                is_triple = true;
                sanitized.push_str("   ");
                i += 3;
                continue;
            }

            // Check single quote string
            if c == '"' || c == '\'' {
                in_string = true;
                quote_char = c;
                is_triple = false;
                sanitized.push(' ');
                i += 1;
                continue;
            }

            // Check comment
            if c == '#' {
                while i < chars.len() && chars[i] != '\n' {
                    sanitized.push(' ');
                    i += 1;
                }
                continue;
            }

            sanitized.push(c);
            i += 1;
        }

        sanitized
    }

    fn strip_comments_and_strings(source: &str, _is_py: bool) -> String {
        let (sanitized, _) = Self::strip_c_style_comments_with_docs(source);
        sanitized
    }

    fn compute_brace_end(source: &str, start_idx: usize) -> usize {
        let lines: Vec<&str> = source.lines().collect();
        let mut depth = 0;
        let mut seen_open = false;

        for (offset, line) in lines.iter().skip(start_idx).enumerate() {
            for c in line.chars() {
                if c == '{' {
                    depth += 1;
                    seen_open = true;
                } else if c == '}' {
                    depth -= 1;
                    if seen_open && depth <= 0 {
                        return start_idx + offset + 1;
                    }
                }
            }
        }
        start_idx + 1
    }

    fn compute_indent_end(lines: &[&str], start_idx: usize) -> usize {
        if start_idx >= lines.len() {
            return start_idx + 1;
        }

        let header_indent = lines[start_idx]
            .chars()
            .take_while(|c| c.is_whitespace())
            .count();
        let mut end_idx = start_idx + 1;

        for (offset, line) in lines.iter().skip(start_idx + 1).enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let current_indent = line.chars().take_while(|c| c.is_whitespace()).count();

            if current_indent <= header_indent {
                break;
            }
            end_idx = start_idx + 1 + offset + 1;
        }

        end_idx
    }

    fn extract_python_docstring(lines: &[&str], fn_idx: usize) -> Option<String> {
        if fn_idx + 1 >= lines.len() {
            return None;
        }
        let next_line = lines[fn_idx + 1].trim();
        if next_line.starts_with("\"\"\"") || next_line.starts_with("'''") {
            let quote = &next_line[0..3];
            let rest = &next_line[3..];
            if rest.contains(quote) {
                return Some(rest.trim_end_matches(quote).trim().to_string());
            }
            let mut doc = rest.to_string();
            for line in lines.iter().skip(fn_idx + 2) {
                if line.contains(quote) {
                    doc.push('\n');
                    doc.push_str(line.split(quote).next().unwrap_or(""));
                    break;
                }
                doc.push('\n');
                doc.push_str(line);
            }
            return Some(doc.trim().to_string());
        }
        None
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_ast_ignores_comments_and_strings() {
        let rust_code = r#"
        // fn fake_in_comment() -> bool { false }
        /* 
           fn multiline_comment_fn() {}
        */

        /// Documentation for real function
        pub fn real_function(a: u32, b: &str) -> Result<(), ()> {
            let fake_in_string = "fn fake_inside_str() {}";
            Ok(())
        }

        pub struct UserAccount {
            pub id: u64,
        }

        impl UserAccount {
            pub fn get_id(&self) -> u64 {
                self.id
            }
        }
        "#;

        let symbols = SyntaxEngine::extract_symbols(rust_code, SupportedLanguage::Rust);

        // Assert fake functions in comments and strings were NOT indexed
        assert!(!symbols.iter().any(|s| s.name == "fake_in_comment"));
        assert!(!symbols.iter().any(|s| s.name == "multiline_comment_fn"));
        assert!(!symbols.iter().any(|s| s.name == "fake_inside_str"));

        // Assert real symbols WERE indexed
        let real_fn = symbols
            .iter()
            .find(|s| s.name == "real_function")
            .expect("real_function missing");
        assert_eq!(real_fn.kind, "function");
        assert!(real_fn
            .docstring
            .as_ref()
            .unwrap()
            .contains("Documentation for real function"));

        let real_struct = symbols
            .iter()
            .find(|s| s.name == "UserAccount")
            .expect("UserAccount missing");
        assert_eq!(real_struct.kind, "struct");

        let method = symbols
            .iter()
            .find(|s| s.name == "get_id")
            .expect("get_id missing");
        assert_eq!(method.kind, "method");
    }

    #[test]
    fn test_typescript_ignores_comments_and_strings() {
        let ts_code = r#"
        // function fakeInComment(): void {}
        /* function multilineComment() {} */

        /**
         * Real TypeScript calculate total
         */
        export function calculateTotal(price: number, tax: number): number {
            const str = "function insideString() {}";
            return price + tax;
        }

        export class OrderManager {
            process(): void {}
        }

        export interface UserProfile {
            name: string;
        }
        "#;

        let symbols = SyntaxEngine::extract_symbols(ts_code, SupportedLanguage::TypeScript);

        assert!(!symbols.iter().any(|s| s.name == "fakeInComment"));
        assert!(!symbols.iter().any(|s| s.name == "multilineComment"));
        assert!(!symbols.iter().any(|s| s.name == "insideString"));

        let fn_sym = symbols
            .iter()
            .find(|s| s.name == "calculateTotal")
            .expect("calculateTotal missing");
        assert_eq!(fn_sym.kind, "function");
        assert!(fn_sym
            .docstring
            .as_ref()
            .unwrap()
            .contains("Real TypeScript calculate total"));

        assert!(symbols
            .iter()
            .any(|s| s.name == "OrderManager" && s.kind == "class"));
        assert!(symbols
            .iter()
            .any(|s| s.name == "UserProfile" && s.kind == "interface"));
    }

    #[test]
    fn test_python_ignores_comments_and_strings() {
        let py_code = r#"
# def fake_comment(): pass

def real_python_func(x, y):
    """Docstring for real python function"""
    fake_str = "def fake_in_string(): pass"
    return x + y

class DataService:
    """Service class"""
    pass
        "#;

        let symbols = SyntaxEngine::extract_symbols(py_code, SupportedLanguage::Python);

        assert!(!symbols.iter().any(|s| s.name == "fake_comment"));
        assert!(!symbols.iter().any(|s| s.name == "fake_in_string"));

        let fn_sym = symbols
            .iter()
            .find(|s| s.name == "real_python_func")
            .expect("real_python_func missing");
        assert_eq!(fn_sym.kind, "function");
        assert_eq!(
            fn_sym.docstring.as_deref(),
            Some("Docstring for real python function")
        );

        let class_sym = symbols
            .iter()
            .find(|s| s.name == "DataService")
            .expect("DataService missing");
        assert_eq!(class_sym.kind, "class");
        assert_eq!(class_sym.docstring.as_deref(), Some("Service class"));
    }
}
