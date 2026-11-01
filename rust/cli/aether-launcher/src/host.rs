use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct HostConfig {
    pub workbench_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub session_token: Option<String>,
    pub peer_id: Option<String>,
    pub daemon_port: u16,
    pub initial_path: Option<PathBuf>,
}

/// Cross-platform abstraction for desktop native webview hosting.
/// Windows: Win32 HWND + Microsoft WebView2
/// Linux: GTK3 / WebKitGTK
/// macOS: Cocoa / WKWebView
pub trait DesktopHost {
    fn run(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn platform_name(&self) -> &'static str;
}
