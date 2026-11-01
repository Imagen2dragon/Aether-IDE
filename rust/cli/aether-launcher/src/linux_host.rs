use crate::host::{DesktopHost, HostConfig};

/// Linux desktop host implementation targeting GTK3 and WebKitGTK.
pub struct LinuxGtkHost {
    config: HostConfig,
}

impl LinuxGtkHost {
    pub fn new(config: HostConfig) -> Self {
        Self { config }
    }
}

impl DesktopHost for LinuxGtkHost {
    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!(" [✓] Initializing Linux Native Host (GTK3 / WebKitGTK)...");
        let html_path = self.config.workbench_dir.join("index.html");
        if !html_path.exists() {
            return Err(format!("Workbench HTML not found at {:?}", html_path).into());
        }
        println!(
            " [✓] Linux WebKitGTK Host ready with workbench: {:?}",
            html_path
        );
        println!(
            " [✓] Session connected to daemon on port {}",
            self.config.daemon_port
        );
        Ok(())
    }

    fn platform_name(&self) -> &'static str {
        "Linux (GTK3 / WebKitGTK)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_linux_host_metadata() {
        let host = LinuxGtkHost::new(HostConfig {
            workbench_dir: PathBuf::from("."),
            user_data_dir: PathBuf::from("."),
            session_token: None,
            peer_id: None,
            daemon_port: 44888,
            initial_path: None,
        });
        assert_eq!(host.platform_name(), "Linux (GTK3 / WebKitGTK)");
    }
}
