use serde::{Deserialize, Serialize};

pub const AETHER_MDNS_SERVICE_TYPE: &str = "_aether-sync._tcp.local.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncServiceInfo {
    pub instance_name: String,
    pub host: String,
    pub port: u16,
    pub version: String,
    pub session_id: String,
}

impl SyncServiceInfo {
    pub fn new(instance_name: &str, host: &str, port: u16, session_id: &str) -> Self {
        Self {
            instance_name: instance_name.to_string(),
            host: host.to_string(),
            port,
            version: application_core::AETHER_VERSION.to_string(),
            session_id: session_id.to_string(),
        }
    }

    pub fn to_txt_records(&self) -> Vec<(String, String)> {
        vec![
            ("version".to_string(), self.version.clone()),
            ("session".to_string(), self.session_id.clone()),
            ("service".to_string(), "aether-ide".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_service_info() {
        let info = SyncServiceInfo::new("MacBook-Sync", "192.168.1.100", 44999, "sess-123");
        assert_eq!(info.port, 44999);
        let records = info.to_txt_records();
        assert!(records
            .iter()
            .any(|(k, v)| k == "version" && v == application_core::AETHER_VERSION));
    }
}
