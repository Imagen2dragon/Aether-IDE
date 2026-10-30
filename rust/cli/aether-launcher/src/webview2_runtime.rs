use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use tracing::info;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone)]
pub struct WebView2RuntimeInfo {
    pub is_available: bool,
    pub version: Option<String>,
    pub executable_path: Option<PathBuf>,
}

pub fn detect_webview2_runtime() -> WebView2RuntimeInfo {
    // 1. Check known binary paths
    let candidate_paths = [
        PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
        PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
    ];

    for path in &candidate_paths {
        if path.exists() {
            let version = query_file_version(path);
            return WebView2RuntimeInfo {
                is_available: true,
                version,
                executable_path: Some(path.clone()),
            };
        }
    }

    // 2. Check local appdata installation
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let user_edge = PathBuf::from(local_appdata)
            .join("Microsoft")
            .join("Edge")
            .join("Application")
            .join("msedge.exe");
        if user_edge.exists() {
            let version = query_file_version(&user_edge);
            return WebView2RuntimeInfo {
                is_available: true,
                version,
                executable_path: Some(user_edge),
            };
        }
    }

    // 3. Fallback check: "msedge.exe" on PATH
    if let Ok(output) = Command::new("where").arg("msedge.exe").output() {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = out_str.lines().next() {
                let p = PathBuf::from(first_line.trim());
                if p.exists() {
                    let version = query_file_version(&p);
                    return WebView2RuntimeInfo {
                        is_available: true,
                        version,
                        executable_path: Some(p),
                    };
                }
            }
        }
    }

    WebView2RuntimeInfo {
        is_available: false,
        version: None,
        executable_path: None,
    }
}

fn query_file_version(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        let ps_cmd = format!(
            "(Get-Item '{}').VersionInfo.ProductVersion",
            path.to_string_lossy().replace('\'', "''")
        );
        if let Ok(out) = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .output()
        {
            if out.status.success() {
                let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

pub fn launch_webview2_host(
    runtime: &WebView2RuntimeInfo,
    workbench_html_path: &Path,
    data_dir: &Path,
) -> std::io::Result<Child> {
    let edge_exe = runtime.executable_path.as_ref().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "WebView2 / Edge executable not found on host",
        )
    })?;

    let profile_dir = data_dir.join("webview_profile");
    let _ = std::fs::create_dir_all(&profile_dir);

    let html_url = format!(
        "file:///{}",
        workbench_html_path.to_string_lossy().replace('\\', "/")
    );
    let app_flag = format!("--app={}", html_url);
    let user_data_flag = format!("--user-data-dir={}", profile_dir.to_string_lossy());

    let mut cmd = Command::new(edge_exe);
    cmd.arg(app_flag)
        .arg(user_data_flag)
        .arg("--window-size=1440,900")
        .arg("--window-name=AetherIDE")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-sync")
        .arg("--disable-features=Translate");

    #[cfg(windows)]
    {
        // Don't pop up any auxiliary command consoles
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    info!(
        "Launching native desktop host with WebView2 runtime: {:?}",
        edge_exe
    );
    cmd.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_webview2_runtime() {
        let info = detect_webview2_runtime();
        assert!(
            info.is_available,
            "Microsoft Edge / WebView2 runtime should be available on Windows"
        );
        assert!(info.executable_path.is_some());
        assert!(info.executable_path.unwrap().exists());
    }
}
