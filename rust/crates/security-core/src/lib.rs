use regex::Regex;
use std::sync::OnceLock;

static SECRET_PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();

fn get_patterns() -> &'static Vec<(&'static str, Regex)> {
    SECRET_PATTERNS.get_or_init(|| {
        vec![
            ("OPENAI_KEY", Regex::new(r"sk-[A-Za-z0-9]{32,}").unwrap()),
            ("ANTHROPIC_KEY", Regex::new(r"sk-ant-[A-Za-z0-9\-_]{32,}").unwrap()),
            ("GITHUB_TOKEN", Regex::new(r"(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36}").unwrap()),
            ("AWS_ACCESS_KEY", Regex::new(r"AKIA[0-9A-Z]{16}").unwrap()),
            ("PRIVATE_KEY", Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----").unwrap()),
            ("JWT_TOKEN", Regex::new(r"eyJ[A-Za-z0-9-_=]+\.[A-Za-z0-9-_=]+\.?[A-Za-z0-9-_.+/=]*").unwrap()),
        ]
    })
}

pub struct SecretRedactor;

impl SecretRedactor {
    pub fn redact(input: &str) -> String {
        let mut result = input.to_string();
        for (name, re) in get_patterns() {
            result = re.replace_all(&result, format!("[REDACTED_{}]", name)).to_string();
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

    pub fn verify_token(expected: &str, provided: Option<&str>) -> bool {
        match provided {
            Some(token) => token == expected,
            None => false,
        }
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
}
