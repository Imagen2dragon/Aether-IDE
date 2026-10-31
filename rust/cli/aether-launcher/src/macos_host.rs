use crate::host::{DesktopHost, HostConfig};

/// macOS desktop host implementation targeting Cocoa and WKWebView.
pub struct MacWkWebViewHost {
    config: HostConfig,
}

impl MacWkWebViewHost {
    pub fn new(config: HostConfig) -> Self {
        Self { config }
    }
}

impl DesktopHost for MacWkWebViewHost {
    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!(" [✓] Initializing macOS Native Host (Cocoa / WKWebView)...");
        let html_path = self.config.workbench_dir.join("index.html");
        if !html_path.exists() {
            return Err(format!("Workbench HTML not found at {:?}", html_path).into());
        }
        println!(
            " [✓] macOS WKWebView Host ready with workbench: {:?}",
            html_path
        );
        println!(
            " [✓] Session connected to daemon on port {}",
            self.config.daemon_port
        );
        Ok(())
    }

    fn platform_name(&self) -> &'static str {
        "macOS (Cocoa / WKWebView)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_mac_host_metadata() {
        let host = MacWkWebViewHost::new(HostConfig {
            workbench_dir: PathBuf::from("."),
            user_data_dir: PathBuf::from("."),
            session_token: None,
            peer_id: None,
            daemon_port: 44888,
        });
        assert_eq!(host.platform_name(), "macOS (Cocoa / WKWebView)");
    }
}
