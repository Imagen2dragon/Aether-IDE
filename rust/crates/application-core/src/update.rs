use crate::{AetherError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AetherUpdateManifest {
    pub version: String,
    pub channel: String,
    pub release_notes: String,
    pub download_url: String,
    pub sha256_checksum: String,
    pub signature_base64: String,
    pub min_os_version: Option<String>,
    pub published_at: DateTime<Utc>,
}

pub struct UpdateVerifier;

impl UpdateVerifier {
    /// Parses a semantic version string (e.g. "0.2.0" or "v0.1.0-alpha") into (major, minor, patch).
    pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
        let clean = v.trim().trim_start_matches('v');
        let core = clean.split('-').next()?;
        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() < 3 {
            return None;
        }
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;
        Some((major, minor, patch))
    }

    /// Validates an update manifest against current installed version and security requirements.
    pub fn verify_manifest(current_version: &str, manifest: &AetherUpdateManifest) -> Result<()> {
        let curr = Self::parse_semver(current_version).ok_or_else(|| {
            AetherError::Internal(format!("Invalid current semver: {current_version}"))
        })?;
        let target = Self::parse_semver(&manifest.version).ok_or_else(|| {
            AetherError::Internal(format!("Invalid target semver: {}", manifest.version))
        })?;

        // Rollback protection: Target version must be strictly greater than current
        if target <= curr {
            return Err(AetherError::PermissionDenied(format!(
                "Rollback protection: cannot downgrade or reinstall version {} (current is {})",
                manifest.version, current_version
            )));
        }

        // SHA-256 Checksum format validation
        if manifest.sha256_checksum.len() != 64
            || !manifest
                .sha256_checksum
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            return Err(AetherError::Internal(
                "Invalid SHA-256 checksum format in update manifest".into(),
            ));
        }

        // Signature presence check
        if manifest.signature_base64.trim().is_empty() {
            return Err(AetherError::PermissionDenied(
                "Update manifest missing required cryptographic signature".into(),
            ));
        }

        Ok(())
    }

    /// Verifies the cryptographic SHA-256 hash of a downloaded installer payload.
    pub fn verify_payload_checksum(payload: &[u8], expected_sha256: &str) -> Result<()> {
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let hash = hasher.finalize();
        let computed = format!("{:x}", hash);

        if computed.eq_ignore_ascii_case(expected_sha256.trim()) {
            Ok(())
        } else {
            Err(AetherError::Internal(format!(
                "Payload checksum verification failed! Expected {}, computed {}",
                expected_sha256, computed
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_parsing() {
        assert_eq!(UpdateVerifier::parse_semver("0.1.0"), Some((0, 1, 0)));
        assert_eq!(
            UpdateVerifier::parse_semver("v1.2.34-alpha"),
            Some((1, 2, 34))
        );
        assert_eq!(UpdateVerifier::parse_semver("invalid"), None);
    }

    #[test]
    fn test_update_manifest_verification() {
        let valid_manifest = AetherUpdateManifest {
            version: "0.2.0".into(),
            channel: "stable".into(),
            release_notes: "Sprint 06 Release".into(),
            download_url: "https://releases.aetheride.dev/v0.2.0/setup.exe".into(),
            sha256_checksum: "a".repeat(64),
            signature_base64: "c2lnbmF0dXJl".into(),
            min_os_version: Some("10.0.19041".into()),
            published_at: Utc::now(),
        };

        // Upgrade from 0.1.0 to 0.2.0 -> Succeeds
        assert!(UpdateVerifier::verify_manifest("0.1.0", &valid_manifest).is_ok());

        // Rollback attempt: downgrade from 0.3.0 to 0.2.0 -> Rejected
        assert!(UpdateVerifier::verify_manifest("0.3.0", &valid_manifest).is_err());

        // Reinstall attempt: same version 0.2.0 -> Rejected
        assert!(UpdateVerifier::verify_manifest("0.2.0", &valid_manifest).is_err());
    }

    #[test]
    fn test_payload_checksum_verification() {
        let payload = b"AetherIDE Installer Binary Bytes Payload 2026";
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let expected_hash = format!("{:x}", hasher.finalize());

        assert!(UpdateVerifier::verify_payload_checksum(payload, &expected_hash).is_ok());
        assert!(UpdateVerifier::verify_payload_checksum(payload, "invalidhash").is_err());
    }
}
