use regex::Regex;
use sha2::{Digest, Sha256};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthVerificationResult {
    Success,
    InvalidToken,
    Expired,
    RateLimited { wait_seconds: u64 },
}

/// Robust session token store with time-to-live (TTL), rotation with grace period,
/// and rate-limiting against brute force / timing analysis attacks.
pub struct TokenStore {
    current_token: String,
    created_at: Instant,
    ttl: Duration,
    previous_token: Option<(String, Instant)>,
    grace_period: Duration,
    failed_attempts: u32,
    max_failed_attempts: u32,
    last_failed_attempt: Option<Instant>,
    lockout_duration: Duration,
}

impl Default for TokenStore {
    fn default() -> Self {
        Self::new(Duration::from_secs(86400)) // 24-hour default TTL
    }
}

impl TokenStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            current_token: TokenManager::generate_session_token(),
            created_at: Instant::now(),
            ttl,
            previous_token: None,
            grace_period: Duration::from_secs(300), // 5-minute rotation grace period
            failed_attempts: 0,
            max_failed_attempts: 5,
            last_failed_attempt: None,
            lockout_duration: Duration::from_secs(30),
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

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// Rotates the active session token, preserving the old token for the grace period.
    pub fn rotate_token(&mut self) -> String {
        self.previous_token = Some((self.current_token.clone(), Instant::now()));
        self.current_token = TokenManager::generate_session_token();
        self.created_at = Instant::now();
        self.failed_attempts = 0;
        self.current_token.clone()
    }

    /// Authenticates a token request with constant-time comparison, expiry enforcement,
    /// and automatic brute-force lockout.
    pub fn verify(&mut self, provided: Option<&str>) -> AuthVerificationResult {
        // Enforce lockout if max failed attempts exceeded
        if self.failed_attempts >= self.max_failed_attempts {
            if let Some(last_fail) = self.last_failed_attempt {
                let elapsed = last_fail.elapsed();
                if elapsed < self.lockout_duration {
                    let remaining = self.lockout_duration.saturating_sub(elapsed).as_secs();
                    return AuthVerificationResult::RateLimited {
                        wait_seconds: remaining.max(1),
                    };
                } else {
                    // Lockout window expired, reset failed counter
                    self.failed_attempts = 0;
                }
            }
        }

        let token = match provided {
            Some(t) => t,
            None => {
                self.record_failure();
                return AuthVerificationResult::InvalidToken;
            }
        };

        // Check active token
        if TokenManager::verify_token(&self.current_token, Some(token)) {
            if self.is_expired() {
                return AuthVerificationResult::Expired;
            }
            self.failed_attempts = 0;
            return AuthVerificationResult::Success;
        }

        // Check rotated token within grace period
        if let Some((ref prev, rotated_at)) = self.previous_token {
            if rotated_at.elapsed() <= self.grace_period
                && TokenManager::verify_token(prev, Some(token))
            {
                self.failed_attempts = 0;
                return AuthVerificationResult::Success;
            }
        }

        self.record_failure();
        AuthVerificationResult::InvalidToken
    }

    fn record_failure(&mut self) {
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        self.last_failed_attempt = Some(Instant::now());
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
}
