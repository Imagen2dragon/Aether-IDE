use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cursor position in 1-based (line, column) editor coordinates.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CursorPosition {
    pub line: u32,
    pub col: u32,
}

/// Selection range span across lines and columns.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectionRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Ephemeral real-time collaborator state and cursor awareness.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerPresence {
    pub peer_id: String,
    pub display_name: String,
    pub user_color: String,
    pub current_file: Option<String>,
    pub cursor: Option<CursorPosition>,
    pub selection: Option<SelectionRange>,
    pub last_active_utc: DateTime<Utc>,
}

impl PeerPresence {
    pub fn new(
        peer_id: impl Into<String>,
        display_name: impl Into<String>,
        user_color: impl Into<String>,
    ) -> Self {
        Self {
            peer_id: peer_id.into(),
            display_name: display_name.into(),
            user_color: user_color.into(),
            current_file: None,
            cursor: None,
            selection: None,
            last_active_utc: Utc::now(),
        }
    }
}

/// In-memory tracker of connected peer presence and cursor tracking.
#[derive(Default)]
pub struct AwarenessTracker {
    peers: RwLock<HashMap<String, PeerPresence>>,
}

impl AwarenessTracker {
    pub fn new() -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
        }
    }

    /// Update or register peer presence.
    pub fn update_presence(&self, mut presence: PeerPresence) {
        presence.last_active_utc = Utc::now();
        self.peers
            .write()
            .insert(presence.peer_id.clone(), presence);
    }

    /// Remove a disconnected peer immediately.
    pub fn remove_peer(&self, peer_id: &str) -> Option<PeerPresence> {
        self.peers.write().remove(peer_id)
    }

    /// Return all peers active within the specified timeout in seconds.
    pub fn get_active_peers(&self, timeout_secs: i64) -> Vec<PeerPresence> {
        let threshold = Utc::now() - Duration::seconds(timeout_secs);
        self.peers
            .read()
            .values()
            .filter(|p| p.last_active_utc >= threshold)
            .cloned()
            .collect()
    }

    /// Return peers actively viewing or editing a specific document path.
    pub fn get_peers_for_file(&self, file_path: &str, timeout_secs: i64) -> Vec<PeerPresence> {
        let threshold = Utc::now() - Duration::seconds(timeout_secs);
        self.peers
            .read()
            .values()
            .filter(|p| {
                p.last_active_utc >= threshold && p.current_file.as_deref() == Some(file_path)
            })
            .cloned()
            .collect()
    }

    /// Prunes peers whose last heartbeat exceeded the timeout window.
    pub fn prune_inactive(&self, timeout_secs: i64) -> usize {
        let threshold = Utc::now() - Duration::seconds(timeout_secs);
        let mut peers = self.peers.write();
        let initial = peers.len();
        peers.retain(|_, p| p.last_active_utc >= threshold);
        initial.saturating_sub(peers.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_awareness_tracking_and_file_filtering() {
        let tracker = AwarenessTracker::new();

        let mut peer1 = PeerPresence::new("p1", "Alice", "#3b82f6");
        peer1.current_file = Some("src/main.rs".to_string());
        peer1.cursor = Some(CursorPosition { line: 12, col: 5 });

        let mut peer2 = PeerPresence::new("p2", "Bob", "#10b981");
        peer2.current_file = Some("src/lib.rs".to_string());

        tracker.update_presence(peer1);
        tracker.update_presence(peer2);

        let active = tracker.get_active_peers(30);
        assert_eq!(active.len(), 2);

        let file_peers = tracker.get_peers_for_file("src/main.rs", 30);
        assert_eq!(file_peers.len(), 1);
        assert_eq!(file_peers[0].display_name, "Alice");
        assert_eq!(file_peers[0].cursor.as_ref().unwrap().line, 12);
    }
}
