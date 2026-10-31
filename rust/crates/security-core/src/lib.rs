use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static SECRET_PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();

fn get_patterns() -> &'static Vec<(&'static str, Regex)> {
    SECRET_PATTERNS.get_or_init(|| {
        vec![
            ("OPENAI_KEY", Regex::new(r"sk-[A-Za-z0-9]{32,}").unwrap()),
            (
                "ANTHROPIC_KEY",
                Regex::new(r"sk-ant-[A-Za-z0-9\-_]{32,}").unwrap(),
            ),
            (
                "GITHUB_TOKEN",
                Regex::new(r"(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36}").unwrap(),
            ),
            ("AWS_ACCESS_KEY", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
            (
                "PRIVATE_KEY",
                Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----").unwrap(),
            ),
            (
                "JWT_TOKEN",
                Regex::new(r"eyJ[A-Za-z0-9-_=]+\.[A-Za-z0-9-_=]+\.?[A-Za-z0-9-_.+/=]*").unwrap(),
            ),
        ]
    })
}

pub struct SecretRedactor;

impl SecretRedactor {
    pub fn redact(input: &str) -> String {
        let mut result = input.to_string();
        for (name, re) in get_patterns() {
            result = re
                .replace_all(&result, format!("[REDACTED_{}]", name))
                .to_string();
        }
        result
    }

    pub fn contains_secret(input: &str) -> bool {
        for (_, re) in get_patterns() {
            if re.is_match(input) {
                return true;
            }
        }
        false
    }
}

pub struct TokenManager;

impl TokenManager {
    pub fn generate_session_token() -> String {
        format!("aether-sess-{}", uuid::Uuid::new_v4().simple())
    }

    /// Verifies session tokens in constant-time using cryptographic SHA-256 digests.
    /// Eliminates both character-value and length-based timing side-channel leakage.
    pub fn verify_token(expected: &str, provided: Option<&str>) -> bool {
        match provided {
            Some(token) => {
                let hash_expected = Sha256::digest(expected.as_bytes());
                let hash_provided = Sha256::digest(token.as_bytes());

                let mut diff = 0u8;
                for (a, b) in hash_expected.iter().zip(hash_provided.iter()) {
                    diff |= a ^ b;
                }
                diff == 0
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SessionType {
    DesktopOwner,
    LocalDesktopClient,
    RemoteCompanion,
    Collaborator,
}

impl std::fmt::Display for SessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DesktopOwner => write!(f, "DesktopOwner"),
            Self::LocalDesktopClient => write!(f, "LocalDesktopClient"),
            Self::RemoteCompanion => write!(f, "RemoteCompanion"),
            Self::Collaborator => write!(f, "Collaborator"),
        }
    }
}

/// Server-controlled session record binding an authenticated token to a cryptographic identity and role.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionRecord {
    pub session_token: String,
    pub peer_id: String,
    pub role: String,
    pub session_type: SessionType,
    #[serde(skip, default = "Instant::now")]
    pub created_at: Instant,
    pub ttl: Duration,
}

impl SessionRecord {
    pub fn new(
        session_token: impl Into<String>,
        peer_id: impl Into<String>,
        role: impl Into<String>,
        session_type: SessionType,
        ttl: Duration,
    ) -> Self {
        Self {
            session_token: session_token.into(),
            peer_id: peer_id.into(),
            role: role.into(),
            session_type,
            created_at: Instant::now(),
            ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthVerificationResult {
    Success,
    InvalidToken,
    Expired,
    Revoked,
    RateLimited { wait_seconds: u64 },
}

/// Robust session token store with time-to-live (TTL), rotation with grace period,
/// token revocation, per-source rate-limiting, and server-controlled session binding.
pub struct TokenStore {
    current_token: String,
    created_at: Instant,
    ttl: Duration,
    previous_token: Option<(String, Instant)>,
    grace_period: Duration,
    revoked_tokens: HashSet<String>,
    failed_attempts_by_source: HashMap<String, (u32, Instant)>,
    max_failed_attempts: u32,
    lockout_duration: Duration,
    owner_peer_id: String,
    sessions: HashMap<String, SessionRecord>,
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new(Duration::from_secs(86400)) // 24-hour default TTL
    }
}

impl TokenStore {
    pub fn new(ttl: Duration) -> Self {
        Self::new_with_owner(ttl, "owner-default")
    }

    pub fn new_with_owner(ttl: Duration, owner_peer_id: impl Into<String>) -> Self {
        let current_token = TokenManager::generate_session_token();
        let owner_id = owner_peer_id.into();
        let mut sessions = HashMap::new();
        sessions.insert(
            current_token.clone(),
            SessionRecord::new(
                &current_token,
                &owner_id,
                "Owner",
                SessionType::DesktopOwner,
                ttl,
            ),
        );

        Self {
            current_token,
            created_at: Instant::now(),
            ttl,
            previous_token: None,
            grace_period: Duration::from_secs(300), // 5-minute rotation grace period
            revoked_tokens: HashSet::new(),
            failed_attempts_by_source: HashMap::new(),
            max_failed_attempts: 5,
            lockout_duration: Duration::from_secs(30),
            owner_peer_id: owner_id,
            sessions,
        }
    }

    pub fn with_lockout(mut self, max_attempts: u32, lockout: Duration) -> Self {
        self.max_failed_attempts = max_attempts;
        self.lockout_duration = lockout;
        self
    }

    pub fn current_token(&self) -> &str {
        &self.current_token
    }

    pub fn owner_peer_id(&self) -> &str {
        &self.owner_peer_id
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts_for_source("default")
    }

    pub fn failed_attempts_for_source(&self, source: &str) -> u32 {
        self.failed_attempts_by_source
            .get(source)
            .map(|(count, _)| *count)
            .unwrap_or(0)
    }

    /// Registers a server-controlled session record binding token to identity and role.
    pub fn register_session(&mut self, session: SessionRecord) {
        self.sessions.insert(session.session_token.clone(), session);
    }

    /// Retrieves an active session record by its token.
    pub fn get_session(&self, token: &str) -> Option<&SessionRecord> {
        self.sessions.get(token)
    }

    /// Revokes a session token immediately and purges its session record.
    pub fn revoke_token(&mut self, token: impl Into<String>) {
        let t = token.into();
        self.sessions.remove(&t);
        self.revoked_tokens.insert(t);
    }

    /// Revokes all active sessions bound to a specific peer ID.
    pub fn revoke_peer_sessions(&mut self, peer_id: &str) -> Vec<String> {
        let mut revoked = Vec::new();
        let tokens_to_revoke: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.peer_id == peer_id)
            .map(|(t, _)| t.clone())
            .collect();

        for token in tokens_to_revoke {
            self.revoke_token(&token);
            revoked.push(token);
        }
        revoked
    }

    pub fn is_revoked(&self, token: &str) -> bool {
        self.revoked_tokens.contains(token)
    }

    /// Rotates the active owner session token, preserving the old token for the grace period
    /// and updating the authoritative server-side session registry.
    pub fn rotate_token(&mut self) -> String {
        self.previous_token = Some((self.current_token.clone(), Instant::now()));
        self.current_token = TokenManager::generate_session_token();
        self.created_at = Instant::now();

        // Update server-side session registry with new active owner session
        let new_owner_session = SessionRecord::new(
            &self.current_token,
            &self.owner_peer_id,
            "Owner",
            SessionType::DesktopOwner,
            self.ttl,
        );
        self.sessions
            .insert(self.current_token.clone(), new_owner_session);

        self.current_token.clone()
    }

    /// Authenticates a token request and returns the authoritative server-side SessionRecord.
    pub fn authenticate_session(
        &mut self,
        provided: Option<&str>,
        source: &str,
    ) -> (AuthVerificationResult, Option<SessionRecord>) {
        let verify_result = self.verify_for_source(provided, source);
        if verify_result != AuthVerificationResult::Success {
            return (verify_result, None);
        }

        let token = match provided {
            Some(t) => t,
            None => return (AuthVerificationResult::InvalidToken, None),
        };

        // First check explicit session registry
        if let Some(session) = self.sessions.get(token) {
            if session.is_expired() {
                return (AuthVerificationResult::Expired, None);
            }
            return (AuthVerificationResult::Success, Some(session.clone()));
        }

        // Check if token matches active or grace-period owner token
        let is_owner_match = TokenManager::verify_token(&self.current_token, Some(token))
            || self.previous_token.as_ref().is_some_and(|(prev, rot)| {
                rot.elapsed() <= self.grace_period && TokenManager::verify_token(prev, Some(token))
            });

        if is_owner_match {
            let owner_record = SessionRecord::new(
                token,
                &self.owner_peer_id,
                "Owner",
                SessionType::DesktopOwner,
                self.ttl,
            );
            return (AuthVerificationResult::Success, Some(owner_record));
        }

        (AuthVerificationResult::InvalidToken, None)
    }

    /// Authenticates a token request for a specific client source.
    pub fn verify_for_source(
        &mut self,
        provided: Option<&str>,
        source: &str,
    ) -> AuthVerificationResult {
        // Enforce lockout if max failed attempts exceeded for this source
        if let Some((attempts, last_fail)) = self.failed_attempts_by_source.get(source) {
            if *attempts >= self.max_failed_attempts {
                let elapsed = last_fail.elapsed();
                if elapsed < self.lockout_duration {
                    let remaining = self.lockout_duration.saturating_sub(elapsed).as_secs();
                    return AuthVerificationResult::RateLimited {
                        wait_seconds: remaining.max(1),
                    };
                }
            }
        }

        let token = match provided {
            Some(t) => t,
            None => {
                self.record_failure(source);
                return AuthVerificationResult::InvalidToken;
            }
        };

        // Reject explicitly revoked tokens
        if self.is_revoked(token) {
            self.record_failure(source);
            return AuthVerificationResult::Revoked;
        }

        // Check active token
        if TokenManager::verify_token(&self.current_token, Some(token)) {
            if self.is_expired() {
                return AuthVerificationResult::Expired;
            }
            self.failed_attempts_by_source.remove(source);
            return AuthVerificationResult::Success;
        }

        // Check registered sessions
        if let Some(session) = self.sessions.get(token) {
            if session.is_expired() {
                return AuthVerificationResult::Expired;
            }
            self.failed_attempts_by_source.remove(source);
            return AuthVerificationResult::Success;
        }

        // Check rotated token within grace period
        if let Some((ref prev, rotated_at)) = self.previous_token {
            if rotated_at.elapsed() <= self.grace_period
                && TokenManager::verify_token(prev, Some(token))
            {
                self.failed_attempts_by_source.remove(source);
                return AuthVerificationResult::Success;
            }
        }

        self.record_failure(source);
        AuthVerificationResult::InvalidToken
    }

    /// Default verification method for backwards compatibility.
    pub fn verify(&mut self, provided: Option<&str>) -> AuthVerificationResult {
        self.verify_for_source(provided, "default")
    }

    fn record_failure(&mut self, source: &str) {
        let entry = self
            .failed_attempts_by_source
            .entry(source.to_string())
            .or_insert((0, Instant::now()));
        entry.0 = entry.0.saturating_add(1);
        entry.1 = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_redactor() {
        let raw = "My key is sk-1234567890123456789012345678901234 and call me later";
        let redacted = SecretRedactor::redact(raw);
        assert!(!redacted.contains("sk-1234567890123456789012345678901234"));
        assert!(redacted.contains("[REDACTED_OPENAI_KEY]"));
    }

    #[test]
    fn test_session_token_verification() {
        let token = TokenManager::generate_session_token();
        assert!(TokenManager::verify_token(&token, Some(&token)));
        assert!(!TokenManager::verify_token(&token, Some("wrong-token")));
        assert!(!TokenManager::verify_token(&token, None));
    }

    #[test]
    fn test_token_store_lifecycle_and_rate_limiting() {
        let mut store =
            TokenStore::new(Duration::from_millis(50)).with_lockout(3, Duration::from_millis(100));

        let original_token = store.current_token().to_string();

        // 1. Success on valid token
        assert_eq!(
            store.verify(Some(&original_token)),
            AuthVerificationResult::Success
        );

        // 2. Failed attempt records correctly
        assert_eq!(
            store.verify(Some("wrong-1")),
            AuthVerificationResult::InvalidToken
        );
        assert_eq!(store.failed_attempts(), 1);

        assert_eq!(
            store.verify(Some("wrong-2")),
            AuthVerificationResult::InvalidToken
        );
        assert_eq!(store.failed_attempts(), 2);

        assert_eq!(
            store.verify(Some("wrong-3")),
            AuthVerificationResult::InvalidToken
        );
        assert_eq!(store.failed_attempts(), 3);

        // 3. Rate limited on subsequent attempt
        match store.verify(Some(&original_token)) {
            AuthVerificationResult::RateLimited { wait_seconds: _ } => {}
            other => panic!("Expected RateLimited, got {:?}", other),
        }

        // Wait for lockout to expire
        std::thread::sleep(Duration::from_millis(120));

        // 4. Token rotation and grace period
        let mut store2 =
            TokenStore::new(Duration::from_secs(3600)).with_lockout(5, Duration::from_secs(10));
        let tok1 = store2.current_token().to_string();
        let tok2 = store2.rotate_token();
        assert_ne!(tok1, tok2);

        // Both new token and rotated token (grace period) verify
        assert_eq!(store2.verify(Some(&tok2)), AuthVerificationResult::Success);
        assert_eq!(store2.verify(Some(&tok1)), AuthVerificationResult::Success);

        // 5. Expiration check
        let mut expired_store = TokenStore::new(Duration::from_millis(10));
        let exp_tok = expired_store.current_token().to_string();
        std::thread::sleep(Duration::from_millis(25));
        assert!(expired_store.is_expired());
        assert_eq!(
            expired_store.verify(Some(&exp_tok)),
            AuthVerificationResult::Expired
        );
    }

    #[test]
    fn test_token_store_revocation() {
        let mut store = TokenStore::new(Duration::from_secs(3600));
        let token = store.current_token().to_string();

        assert_eq!(store.verify(Some(&token)), AuthVerificationResult::Success);

        // Revoke token
        store.revoke_token(&token);
        assert!(store.is_revoked(&token));
        assert_eq!(store.verify(Some(&token)), AuthVerificationResult::Revoked);
    }

    #[test]
    fn test_token_store_per_source_isolation() {
        let mut store =
            TokenStore::new(Duration::from_secs(3600)).with_lockout(3, Duration::from_millis(200));

        let valid_token = store.current_token().to_string();

        // Attacker repeatedly fails from "attacker-source"
        assert_eq!(
            store.verify_for_source(Some("bad-1"), "attacker-source"),
            AuthVerificationResult::InvalidToken
        );
        assert_eq!(
            store.verify_for_source(Some("bad-2"), "attacker-source"),
            AuthVerificationResult::InvalidToken
        );
        assert_eq!(
            store.verify_for_source(Some("bad-3"), "attacker-source"),
            AuthVerificationResult::InvalidToken
        );

        // "attacker-source" is now locked out
        match store.verify_for_source(Some(&valid_token), "attacker-source") {
            AuthVerificationResult::RateLimited { wait_seconds: _ } => {}
            other => panic!("Expected RateLimited for attacker, got {:?}", other),
        }

        // Legitimate client from "desktop-host" is NOT locked out!
        assert_eq!(
            store.verify_for_source(Some(&valid_token), "desktop-host"),
            AuthVerificationResult::Success
        );
    }

    #[test]
    fn test_server_controlled_session_binding() {
        let mut store = TokenStore::new_with_owner(Duration::from_secs(3600), "desktop-owner-uuid");
        let owner_token = store.current_token().to_string();

        // Owner authenticates
        let (auth, session) = store.authenticate_session(Some(&owner_token), "desktop-ui");
        assert_eq!(auth, AuthVerificationResult::Success);
        let session = session.expect("Owner session record must exist");
        assert_eq!(session.peer_id, "desktop-owner-uuid");
        assert_eq!(session.role, "Owner");
        assert_eq!(session.session_type, SessionType::DesktopOwner);

        // Register a distinct Collaborator session with Editor role
        let editor_token = "collab-editor-token-123456";
        let editor_session = SessionRecord::new(
            editor_token,
            "collab-alice",
            "Editor",
            SessionType::Collaborator,
            Duration::from_secs(3600),
        );
        store.register_session(editor_session);

        // Register a distinct Remote Companion session with Viewer role
        let companion_token = "companion-viewer-token-654321";
        let companion_session = SessionRecord::new(
            companion_token,
            "ipad-companion",
            "Viewer",
            SessionType::RemoteCompanion,
            Duration::from_secs(3600),
        );
        store.register_session(companion_session);

        // Authenticate Editor
        let (auth, session) = store.authenticate_session(Some(editor_token), "collab-client");
        assert_eq!(auth, AuthVerificationResult::Success);
        let session = session.expect("Editor session record must exist");
        assert_eq!(session.peer_id, "collab-alice");
        assert_eq!(session.role, "Editor");
        assert_eq!(session.session_type, SessionType::Collaborator);

        // Authenticate Companion Viewer
        let (auth, session) = store.authenticate_session(Some(companion_token), "mobile-ws");
        assert_eq!(auth, AuthVerificationResult::Success);
        let session = session.expect("Viewer session record must exist");
        assert_eq!(session.peer_id, "ipad-companion");
        assert_eq!(session.role, "Viewer");
        assert_eq!(session.session_type, SessionType::RemoteCompanion);

        // Rotate owner token preserves owner binding
        let new_owner_tok = store.rotate_token();
        let (auth, session) = store.authenticate_session(Some(&new_owner_tok), "desktop-ui");
        assert_eq!(auth, AuthVerificationResult::Success);
        let session = session.expect("Rotated owner session must exist");
        assert_eq!(session.peer_id, "desktop-owner-uuid");
        assert_eq!(session.role, "Owner");

        // Grace period old owner token still resolves to Owner
        let (auth, session) = store.authenticate_session(Some(&owner_token), "desktop-ui");
        assert_eq!(auth, AuthVerificationResult::Success);
        assert_eq!(session.unwrap().peer_id, "desktop-owner-uuid");

        // Revoke companion session
        store.revoke_token(companion_token);
        let (auth, session) = store.authenticate_session(Some(companion_token), "mobile-ws");
        assert_eq!(auth, AuthVerificationResult::Revoked);
        assert!(session.is_none());
    }
}
