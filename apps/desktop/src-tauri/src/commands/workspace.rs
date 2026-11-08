use crate::win32_dialogs;
use crate::workspace_bridge::WorkspaceSession;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::HWND;

pub type SharedWorkspace = Arc<parking_lot::Mutex<WorkspaceSession>>;

fn get_parent_hwnd_isize(app: &AppHandle) -> isize {
    app.get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn workspace_open_folder(app: AppHandle, path: Option<String>) -> Result<Value, String> {
    let target_path = match path {
        Some(p) => {
            let pb = PathBuf::from(&p);
            if !pb.exists() {
                return Err(format!("Specified folder does not exist: {}", p));
            }
            pb.canonicalize().unwrap_or(pb)
        }
        None => {
            let hwnd_raw = get_parent_hwnd_isize(&app);
            let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
                let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
                win32_dialogs::pick_folder(hwnd, Some("Select Workspace Folder"), None)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            match picked {
                Some(p) => p,
                None => return Ok(json!({ "cancelled": true })),
            }
        }
    };

    let session_arc = app.state::<SharedWorkspace>();
    let summary = {
        let session = session_arc.lock();
        session.set_single_root(target_path.clone())?
    };

    Ok(json!({
        "success": true,
        "path": target_path.to_string_lossy().to_string(),
        "summary": summary
    }))
}

#[tauri::command]
pub async fn workspace_add_folder(app: AppHandle, path: Option<String>) -> Result<Value, String> {
    let target_path = match path {
        Some(p) => {
            let pb = PathBuf::from(&p);
            if !pb.exists() {
                return Err(format!("Specified folder does not exist: {}", p));
            }
            pb.canonicalize().unwrap_or(pb)
        }
        None => {
            let hwnd_raw = get_parent_hwnd_isize(&app);
            let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
                let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
                win32_dialogs::pick_folder(hwnd, Some("Add Folder to Workspace"), None)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            match picked {
                Some(p) => p,
                None => return Ok(json!({ "cancelled": true })),
            }
        }
    };

    let session_arc = app.state::<SharedWorkspace>();
    let summary = {
        let session = session_arc.lock();
        session.add_root(target_path.clone())?
    };

    Ok(json!({
        "success": true,
        "path": target_path.to_string_lossy().to_string(),
        "summary": summary
    }))
}

#[tauri::command]
pub async fn workspace_get_tree(app: AppHandle) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.get_workspace_summary()
}

#[tauri::command]
pub async fn workspace_open_file(app: AppHandle, path: Option<String>) -> Result<Value, String> {
    let target_path = match path {
        Some(p) => p,
        None => {
            let hwnd_raw = get_parent_hwnd_isize(&app);
            let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
                let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
                win32_dialogs::pick_file(
                    hwnd,
                    Some("Open File"),
                    None,
                    Some("All Files"),
                    Some("*.*"),
                )
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            match picked {
                Some(p) => p.to_string_lossy().to_string(),
                None => return Ok(json!({ "cancelled": true })),
            }
        }
    };

    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    let content = session.read_file(&target_path)?;

    Ok(json!({
        "success": true,
        "path": target_path,
        "content": content
    }))
}

#[tauri::command]
pub async fn workspace_save_file(
    app: AppHandle,
    path: String,
    content: String,
) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    let res = session.write_file(&path, &content)?;
    Ok(res)
}

#[tauri::command]
pub async fn workspace_save_file_as(
    app: AppHandle,
    content: String,
    default_name: Option<String>,
) -> Result<Value, String> {
    let hwnd_raw = get_parent_hwnd_isize(&app);
    let name_clone = default_name.clone();
    let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        win32_dialogs::save_file(
            hwnd,
            Some("Save File As"),
            None,
            name_clone.as_deref().or(Some("untitled.txt")),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    match picked {
        Some(target_path) => {
            let path_str = target_path.to_string_lossy().to_string();
            let session_arc = app.state::<SharedWorkspace>();
            let session = session_arc.lock();
            let res = session.write_file(&path_str, &content)?;
            Ok(res)
        }
        None => Ok(json!({ "cancelled": true })),
    }
}

#[tauri::command]
pub async fn workspace_create_file(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.create_file(&path)
}

#[tauri::command]
pub async fn workspace_create_folder(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.create_directory(&path)
}

#[tauri::command]
pub async fn workspace_rename_entry(
    app: AppHandle,
    old_path: String,
    new_path: String,
) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.rename_entry(&old_path, &new_path)
}

#[tauri::command]
pub async fn workspace_delete_entry(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.delete_entry(&path)
}

#[tauri::command]
pub async fn workspace_reveal_in_explorer(app: AppHandle, path: String) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.reveal_in_explorer(&path)
}

#[tauri::command]
pub async fn workspace_set_trust(
    app: AppHandle,
    path: String,
    trusted: bool,
) -> Result<Value, String> {
    let session_arc = app.state::<SharedWorkspace>();
    let session = session_arc.lock();
    session.set_trust(&path, trusted)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceSearchMatch {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub match_text: String,
    pub line_content: String,
}

thread_local! {
    static INJECT_UNREADABLE_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[doc(hidden)]
pub fn set_inject_unreadable_dir(path: Option<PathBuf>) {
    INJECT_UNREADABLE_DIR.with(|cell| {
        *cell.borrow_mut() = path;
    });
}

pub fn execute_workspace_search(
    root: &Path,
    query: &str,
    is_case_sensitive: bool,
    is_whole_word: bool,
    is_regex: bool,
    max_results: Option<usize>,
) -> Result<Vec<WorkspaceSearchMatch>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let regex_matcher = if is_regex {
        let pattern = if is_whole_word {
            format!(r"\b(?:{})\b", query)
        } else {
            query.to_string()
        };
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(!is_case_sensitive)
            .build()
            .map_err(|e| format!("Invalid regular expression: {}", e))?;
        Some(re)
    } else if is_whole_word {
        let pattern = format!(r"\b{}\b", regex::escape(query));
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(!is_case_sensitive)
            .build()
            .map_err(|e| format!("Regex compilation failed for whole word: {}", e))?;
        Some(re)
    } else if !is_case_sensitive {
        let pattern = regex::escape(query);
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
            .map_err(|e| format!("Regex compilation failed: {}", e))?;
        Some(re)
    } else {
        None
    };

    let max = max_results.unwrap_or(500);
    let mut matches = Vec::new();
    let max_traversal_files = 10_000;
    let mut files_traversed = 0;

    let skip_dirs = [
        ".git",
        "node_modules",
        "target",
        ".system_generated",
        "dist",
        ".aether",
        "build",
        ".idea",
        ".vscode",
        "coverage",
        ".next",
    ];

    let binary_exts = [
        "exe", "dll", "so", "dylib", "png", "jpg", "jpeg", "gif", "ico", "bmp", "zip", "tar", "gz",
        "7z", "pdf", "bin", "lock", "wasm",
    ];

    let canonical_root = match root.canonicalize() {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };

    let mut stack = vec![canonical_root.clone()];
    let mut visited_dirs = std::collections::HashSet::new();
    visited_dirs.insert(canonical_root.clone());

    while let Some(dir) = stack.pop() {
        let is_injected_unreadable = INJECT_UNREADABLE_DIR.with(|cell| {
            if let Some(ref target) = *cell.borrow() {
                &dir == target || dir.ends_with(target)
            } else {
                false
            }
        });

        let entries = if is_injected_unreadable {
            continue; // Emulate genuine OS permission denied error on unreadable directory
        } else {
            match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue, // Safely handle unreadable directories
            }
        };

        for entry in entries.flatten() {
            if files_traversed >= max_traversal_files || matches.len() >= max {
                break;
            }

            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let file_name = entry.file_name().to_string_lossy().to_string();

            if file_type.is_dir() {
                // Critical security: Never follow directory symlinks/junctions to prevent escape/loops
                if file_type.is_symlink() {
                    continue;
                }
                if !skip_dirs.contains(&file_name.as_str()) && !file_name.starts_with('.') {
                    if let Ok(canon) = path.canonicalize() {
                        if canon.starts_with(&canonical_root) && visited_dirs.insert(canon.clone())
                        {
                            stack.push(canon);
                        }
                    }
                }
                continue;
            }

            // File symlink security containment check
            if file_type.is_symlink() {
                match path.canonicalize() {
                    Ok(canon) if canon.starts_with(&canonical_root) => {}
                    _ => continue, // Reject symlink pointing outside authorized root
                }
            }

            files_traversed += 1;

            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if binary_exts.contains(&ext.to_lowercase().as_str()) {
                    continue;
                }
            }

            if let Ok(meta) = entry.metadata() {
                if meta.len() > 4 * 1024 * 1024 {
                    continue;
                }
            }

            // Binary content detection: check initial bytes for null byte (0x00)
            let raw_bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let check_len = raw_bytes.len().min(1024);
            if raw_bytes[..check_len].contains(&0) {
                continue; // Skip binary content
            }

            let content = match String::from_utf8(raw_bytes) {
                Ok(s) => s,
                Err(_) => continue, // Skip non-UTF-8 content
            };

            let rel_file = path
                .strip_prefix(&canonical_root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));

            for (idx, line) in content.lines().enumerate() {
                let line_num = idx + 1;
                if let Some(ref re) = regex_matcher {
                    for m in re.find_iter(line) {
                        matches.push(WorkspaceSearchMatch {
                            file: rel_file.clone(),
                            line: line_num,
                            col: m.start() + 1,
                            match_text: m.as_str().to_string(),
                            line_content: line.trim().to_string(),
                        });
                        if matches.len() >= max {
                            return Ok(matches);
                        }
                    }
                } else {
                    let mut start_idx = 0;
                    while let Some(found_idx) = line[start_idx..].find(query) {
                        let actual_col = start_idx + found_idx + 1;
                        matches.push(WorkspaceSearchMatch {
                            file: rel_file.clone(),
                            line: line_num,
                            col: actual_col,
                            match_text: query.to_string(),
                            line_content: line.trim().to_string(),
                        });
                        start_idx += found_idx + query.len();
                        if matches.len() >= max {
                            return Ok(matches);
                        }
                    }
                }
            }
        }
    }

    Ok(matches)
}

#[tauri::command]
pub async fn workspace_search(
    app: AppHandle,
    query: String,
    is_case_sensitive: bool,
    is_whole_word: bool,
    is_regex: bool,
    max_results: Option<usize>,
) -> Result<Vec<WorkspaceSearchMatch>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let session_arc = app.state::<SharedWorkspace>();
    let mut roots = {
        let session = session_arc.lock();
        session.get_roots()
    };

    if roots.is_empty() {
        if let Ok(cwd) = std::env::current_dir() {
            roots.push(cwd);
        }
    }

    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let max = max_results.unwrap_or(500);
    let mut all_matches = Vec::new();

    for root in roots {
        let remaining = max.saturating_sub(all_matches.len());
        if remaining == 0 {
            break;
        }
        let matches = execute_workspace_search(
            &root,
            &query,
            is_case_sensitive,
            is_whole_word,
            is_regex,
            Some(remaining),
        )?;
        all_matches.extend(matches);
        if all_matches.len() >= max {
            break;
        }
    }

    Ok(all_matches)
}

#[tauri::command]
pub async fn workspace_replace_in_file(
    app: AppHandle,
    file_path: String,
    search_query: String,
    replacement: String,
    is_case_sensitive: bool,
    is_whole_word: bool,
    is_regex: bool,
) -> Result<Value, String> {
    if search_query.is_empty() {
        return Err("Search query cannot be empty for replace".into());
    }

    let session_arc = app.state::<SharedWorkspace>();
    let (cap, _root, rel_path) = {
        let session = session_arc.lock();
        session.resolve_in_workspace(&file_path)?
    };

    let original = cap
        .read_file(&rel_path)
        .map_err(|e| format!("Failed to read file '{}': {}", rel_path, e))?;

    let original_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(original.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let (new_content, count) = if is_regex || is_whole_word {
        let pattern = if is_regex {
            if is_whole_word {
                format!(r"\b{}\b", search_query)
            } else {
                search_query.clone()
            }
        } else {
            format!(r"\b{}\b", regex::escape(&search_query))
        };
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(!is_case_sensitive)
            .build()
            .map_err(|e| format!("Invalid regex: {}", e))?;
        let count = re.find_iter(&original).count();
        let replaced = re.replace_all(&original, replacement.as_str()).to_string();
        (replaced, count)
    } else if is_case_sensitive {
        let count = original.matches(&search_query).count();
        let replaced = original.replace(&search_query, &replacement);
        (replaced, count)
    } else {
        let pattern = regex::escape(&search_query);
        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
            .map_err(|e| format!("Regex build failed: {}", e))?;
        let count = re.find_iter(&original).count();
        let replaced = re.replace_all(&original, replacement.as_str()).to_string();
        (replaced, count)
    };

    if count > 0 {
        // Enforce conditional replacement bound to original_hash (eliminates TOCTOU overwrite race)
        cap.replace_file_bytes_conditional(
            &rel_path,
            new_content.as_bytes(),
            Some(&original_hash),
            None,
        )
        .map_err(|e| {
            format!(
                "Conditional replacement failed for '{}': {}. Target file was modified on disk since search results were generated.",
                rel_path, e
            )
        })?;
    }

    Ok(json!({
        "success": true,
        "file": rel_path,
        "replacements_count": count
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_search_regex_invalid_pattern_rejected() {
        let pattern = "[unclosed_regex_class";
        let res = regex::RegexBuilder::new(pattern).build();
        assert!(
            res.is_err(),
            "Invalid regular expression must fail compilation"
        );
    }

    #[test]
    fn test_replace_with_group_captures() {
        let original = "Hello foo-123 and foo-456 world";
        let pattern = r"foo-(\d+)";
        let replacement = "bar-$1";
        let re = regex::Regex::new(pattern).unwrap();
        let replaced = re.replace_all(original, replacement).to_string();
        assert_eq!(replaced, "Hello bar-123 and bar-456 world");
    }

    #[test]
    fn test_unicode_and_case_insensitive_matching() {
        let original = "AetherIDE supports Rust, 🦀 Unicode, and Case-Insensitive matching.";
        let query = "rust";
        let query_lower = query.to_lowercase();
        let line_lower = original.to_lowercase();
        assert!(line_lower.contains(&query_lower));

        // Exact Unicode match
        assert!(original.contains('🦀'));
    }

    #[test]
    fn test_whole_word_boundary_matching() {
        let original = "cat scatter concatenate cat";
        let pattern = r"\bcat\b";
        let re = regex::Regex::new(pattern).unwrap();
        let matches: Vec<_> = re.find_iter(original).map(|m| m.start()).collect();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], 0);
        assert_eq!(matches[1], 24);
    }

    #[test]
    fn test_binary_null_byte_detection() {
        let text_bytes = b"Hello plain text file without nulls";
        assert!(!text_bytes[..text_bytes.len().min(1024)].contains(&0));

        let mut bin_bytes = b"ELF\0fake_binary_payload".to_vec();
        bin_bytes.resize(2048, 0);
        assert!(bin_bytes[..bin_bytes.len().min(1024)].contains(&0));
    }

    #[test]
    fn test_symlink_containment_verification() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();

        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let outside_file = outside.join("escaped.txt");
        std::fs::write(&outside_file, "escaped content").unwrap();

        let link_path = root.join("link_to_outside.txt");
        #[cfg(windows)]
        {
            let _ = std::os::windows::fs::symlink_file(&outside_file, &link_path);
        }
        #[cfg(not(windows))]
        {
            let _ = std::os::unix::fs::symlink(&outside_file, &link_path);
        }

        if link_path.exists() {
            let canon = link_path.canonicalize().unwrap();
            let canonical_root = root.canonicalize().unwrap();
            // Boundary assertion: symlink outside root must NOT start with canonical root
            assert!(!canon.starts_with(&canonical_root));
        }
    }

    #[test]
    fn test_workspace_search_unicode_case_insensitive_and_byte_offsets() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("unicode.txt");
        // Non-ASCII text with emojis, German sharp S, Greek omega
        let content = "First line\n🦀 Rust is awesome! Großartig und Ωμέγα\nThird line\n";
        std::fs::write(&file, content).unwrap();

        // Search case-insensitively for "großartig"
        let matches =
            super::execute_workspace_search(temp.path(), "großartig", false, false, false, None)
                .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 2);
        assert_eq!(matches[0].match_text, "Großartig");
        // Verify byte slicing: verify that slicing content at [col-1 .. col-1 + match_text.len()] is exact
        let line2 = "🦀 Rust is awesome! Großartig und Ωμέγα";
        let start = matches[0].col - 1;
        let end = start + matches[0].match_text.len();
        assert_eq!(&line2[start..end], "Großartig");

        // Search case-insensitively for "ωμέγα"
        let matches_greek =
            super::execute_workspace_search(temp.path(), "ωμέγα", false, false, false, None)
                .unwrap();
        assert_eq!(matches_greek.len(), 1);
        assert_eq!(matches_greek[0].match_text, "Ωμέγα");
    }

    #[test]
    fn test_workspace_search_whole_word_and_regex() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("code.rs");
        let content = "fn cat() {}\nlet cat = 1;\nlet bobcat = 2;\nlet catch = 3;\n";
        std::fs::write(&file, content).unwrap();

        // Whole word search for "cat"
        let matches =
            super::execute_workspace_search(temp.path(), "cat", true, true, false, None).unwrap();
        assert_eq!(
            matches.len(),
            2,
            "Only standalone 'cat' matches should be found"
        );
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[1].line, 2);

        // Regex search
        let matches_re = super::execute_workspace_search(
            temp.path(),
            r"let \w+ = \d+;",
            true,
            false,
            true,
            None,
        )
        .unwrap();
        assert_eq!(matches_re.len(), 3);

        // Invalid regex rejected
        let res_err = super::execute_workspace_search(
            temp.path(),
            r"[invalid_regex(",
            true,
            false,
            true,
            None,
        );
        assert!(res_err.is_err());
        assert!(res_err.unwrap_err().contains("Invalid regular expression"));
    }

    #[test]
    fn test_workspace_search_binary_files_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let bin_file = temp.path().join("data.dat");
        let mut bin_data = b"Some initial text ".to_vec();
        bin_data.push(0x00); // Null byte
        bin_data.extend_from_slice(b" TARGET_SEARCH_KEYWORD more binary data");
        std::fs::write(&bin_file, bin_data).unwrap();

        let matches = super::execute_workspace_search(
            temp.path(),
            "TARGET_SEARCH_KEYWORD",
            false,
            false,
            false,
            None,
        )
        .unwrap();
        assert_eq!(
            matches.len(),
            0,
            "Binary file with null byte must be skipped"
        );
    }

    #[test]
    fn test_workspace_search_symlink_containment() {
        let temp = tempfile::tempdir().unwrap();
        let internal_file = temp.path().join("internal.txt");
        std::fs::write(&internal_file, "TARGET_INTERNAL_FOUND\n").unwrap();

        let outside_dir = tempfile::tempdir().unwrap();
        let outside_file = outside_dir.path().join("secret.txt");
        std::fs::write(&outside_file, "TARGET_OUTSIDE_SECRET\n").unwrap();

        let link_inside = temp.path().join("link_internal.txt");
        let link_outside = temp.path().join("link_outside.txt");

        #[cfg(windows)]
        {
            let _ = std::os::windows::fs::symlink_file(&internal_file, &link_inside);
            let _ = std::os::windows::fs::symlink_file(&outside_file, &link_outside);
        }
        #[cfg(not(windows))]
        {
            let _ = std::os::unix::fs::symlink(&internal_file, &link_inside);
            let _ = std::os::unix::fs::symlink(&outside_file, &link_outside);
        }

        if link_outside.symlink_metadata().is_ok() {
            let matches_outside = super::execute_workspace_search(
                temp.path(),
                "TARGET_OUTSIDE_SECRET",
                false,
                false,
                false,
                None,
            )
            .unwrap();
            assert_eq!(
                matches_outside.len(),
                0,
                "Symlink pointing outside workspace root must never be traversed"
            );
        }
        if link_inside.symlink_metadata().is_ok() {
            let matches_inside = super::execute_workspace_search(
                temp.path(),
                "TARGET_INTERNAL_FOUND",
                false,
                false,
                false,
                None,
            )
            .unwrap();
            assert!(
                matches_inside.len() >= 1,
                "Internal file/symlink should be matched"
            );
        }
    }

    #[test]
    fn test_workspace_search_directory_symlink_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let real_sub = temp.path().join("real_sub");
        std::fs::create_dir_all(&real_sub).unwrap();
        std::fs::write(real_sub.join("target.txt"), "RECURSIVE_KEYWORD\n").unwrap();

        let symlink_sub = temp.path().join("symlink_dir");
        #[cfg(windows)]
        {
            let _ = std::os::windows::fs::symlink_dir(&real_sub, &symlink_sub);
        }
        #[cfg(not(windows))]
        {
            let _ = std::os::unix::fs::symlink(&real_sub, &symlink_sub);
        }

        let matches = super::execute_workspace_search(
            temp.path(),
            "RECURSIVE_KEYWORD",
            false,
            false,
            false,
            None,
        )
        .unwrap();
        // Should only match from real_sub, NOT duplicated via symlink_dir
        assert_eq!(
            matches.len(),
            1,
            "Directory symlinks must not be followed to avoid loops/duplicate traversal"
        );
    }

    #[test]
    fn test_workspace_search_traversal_limit() {
        let temp = tempfile::tempdir().unwrap();
        for i in 0..20 {
            std::fs::write(temp.path().join(format!("file_{}.txt", i)), "KEYWORD\n").unwrap();
        }
        let matches =
            super::execute_workspace_search(temp.path(), "KEYWORD", false, false, false, Some(5))
                .unwrap();
        assert_eq!(matches.len(), 5, "Search must respect max_results cap");
    }

    #[test]
    fn test_workspace_search_unreadable_directory_handled_gracefully() {
        let temp = tempfile::tempdir().unwrap();

        // Accessible sibling directory and file
        let accessible_dir = temp.path().join("accessible_dir");
        std::fs::create_dir_all(&accessible_dir).unwrap();
        std::fs::write(
            accessible_dir.join("sibling.txt"),
            "ACCESSIBLE_KEYWORD found in sibling\n",
        )
        .unwrap();

        // Locked/unreadable child directory with match that must be skipped
        let locked_dir = temp.path().join("locked_dir");
        std::fs::create_dir_all(&locked_dir).unwrap();
        std::fs::write(
            locked_dir.join("hidden.txt"),
            "ACCESSIBLE_KEYWORD in unreadable child directory\n",
        )
        .unwrap();

        let locked_canon = locked_dir.canonicalize().unwrap();
        super::set_inject_unreadable_dir(Some(locked_canon));

        let matches_res = super::execute_workspace_search(
            temp.path(),
            "ACCESSIBLE_KEYWORD",
            false,
            false,
            false,
            None,
        );

        // Always clean up injected state
        super::set_inject_unreadable_dir(None);

        assert!(
            matches_res.is_ok(),
            "Search must succeed even when child directory is unreadable"
        );
        let matches = matches_res.unwrap();
        assert_eq!(
            matches.len(),
            1,
            "Only accessible sibling file should match; unreadable child directory must be skipped"
        );
        assert!(
            matches[0].file.contains("sibling.txt"),
            "Matched file must be accessible sibling: {}",
            matches[0].file
        );
    }

    #[test]
    fn test_workspace_replace_concurrent_modification_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let cap = filesystem_engine::WorkspaceCapability::open(temp.path()).unwrap();
        let file = temp.path().join("target.txt");
        let initial_bytes = b"Hello original world\n";
        std::fs::write(&file, initial_bytes).unwrap();

        let initial_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(initial_bytes);
            format!("{:x}", hasher.finalize())
        };

        let competing_bytes = b"COMPETING_MODIFICATION_AT_COMMIT_BOUNDARY\n";
        let pre_publish = |dest: &std::path::Path| {
            // Adversary / concurrent worker recreates target at the commit boundary
            // after initial verification passes but before stage publication
            std::fs::write(dest, competing_bytes).unwrap();
        };

        let hooks = filesystem_engine::ConditionalReplaceHooks {
            pre_publish_hook: Some(&pre_publish),
            ..Default::default()
        };

        let res = cap.replace_file_bytes_conditional_with_hooks(
            "target.txt",
            b"Hello proposed replacement\n",
            Some(&initial_hash),
            hooks,
        );

        assert!(
            res.is_err(),
            "Replacement must fail closed when target was modified at commit boundary"
        );
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("Commit boundary conflict"),
            "Error must report commit boundary conflict: {}",
            err_msg
        );

        // Competing modification must remain byte-for-byte intact
        assert_eq!(
            std::fs::read(&file).unwrap(),
            competing_bytes,
            "Competing modification must not be overwritten"
        );

        // Original backup must be preserved in conflict artifact
        let mut conflict_found = false;
        for entry in std::fs::read_dir(temp.path()).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".aether-conflict-") {
                conflict_found = true;
                let conflict_data = std::fs::read(entry.path()).unwrap();
                assert_eq!(
                    conflict_data, initial_bytes,
                    "Conflict file must preserve original content"
                );
            }
            assert!(
                !name.starts_with(".aether-stage-"),
                "No temporary stage files should remain in workspace: {}",
                name
            );
        }
        assert!(
            conflict_found,
            "Conflict artifact must be created to preserve original content"
        );
    }
}
