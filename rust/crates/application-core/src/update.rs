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
    #[serde(default)]
    pub target_os: Option<String>,
    #[serde(default)]
    pub target_arch: Option<String>,
}

pub struct UpdateVerifier;

impl UpdateVerifier {
    /// Returns the canonical message string that is digitally signed by the release authority.
    pub fn canonical_manifest_payload(manifest: &AetherUpdateManifest) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}",
            manifest.version,
            manifest.channel,
            manifest.download_url,
            manifest.sha256_checksum,
            manifest.target_os.as_deref().unwrap_or("any"),
            manifest.target_arch.as_deref().unwrap_or("any"),
        )
    }

    /// Verifies the cryptographic Ed25519 signature of the update manifest against
    /// an authoritative root public key, preventing payload tampering or unauthorized releases.
    pub fn verify_manifest_signature(
        manifest: &AetherUpdateManifest,
        trusted_root_pubkey_base64: &str,
    ) -> Result<()> {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;
        use ring::signature::{UnparsedPublicKey, ED25519};

        let pubkey_bytes = BASE64
            .decode(trusted_root_pubkey_base64.trim())
            .map_err(|e| AetherError::Internal(format!("Invalid root public key base64: {e}")))?;

        let sig_bytes = BASE64
            .decode(manifest.signature_base64.trim())
            .map_err(|e| {
                AetherError::PermissionDenied(format!("Invalid signature base64 in manifest: {e}"))
            })?;

        let canonical_payload = Self::canonical_manifest_payload(manifest);
        let peer_public_key = UnparsedPublicKey::new(&ED25519, &pubkey_bytes);

        peer_public_key
            .verify(canonical_payload.as_bytes(), &sig_bytes)
            .map_err(|_| {
                AetherError::PermissionDenied(
                    "Cryptographic signature verification failed: signature does not match trusted root public key".into(),
                )
            })?;

        // Platform architecture and OS binding validation
        if let Some(ref target_os) = manifest.target_os {
            if target_os != "any" && target_os != std::env::consts::OS {
                return Err(AetherError::PermissionDenied(format!(
                    "Platform mismatch: update manifest is targeted for OS '{}', but current system is '{}'",
                    target_os,
                    std::env::consts::OS
                )));
            }
        }

        if let Some(ref target_arch) = manifest.target_arch {
            if target_arch != "any" && target_arch != std::env::consts::ARCH {
                return Err(AetherError::PermissionDenied(format!(
                    "Architecture mismatch: update manifest is targeted for arch '{}', but current system is '{}'",
                    target_arch,
                    std::env::consts::ARCH
                )));
            }
        }

        Ok(())
    }

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

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StagedUpdateStatus {
    Idle,
    Downloading,
    Downloaded,
    Verified,
    Staged,
    Applying,
    Completed,
    RolledBack,
    Failed,
}

pub struct StagedUpdateApplicator {
    staging_dir: PathBuf,
    installed_dir: PathBuf,
    status: StagedUpdateStatus,
}

impl StagedUpdateApplicator {
    pub fn new(staging_dir: impl Into<PathBuf>, installed_dir: impl Into<PathBuf>) -> Self {
        Self {
            staging_dir: staging_dir.into(),
            installed_dir: installed_dir.into(),
            status: StagedUpdateStatus::Idle,
        }
    }

    pub fn status(&self) -> StagedUpdateStatus {
        self.status
    }

    /// Stages a verified update payload into the staging directory.
    pub fn stage_update(
        &mut self,
        payload: &[u8],
        manifest: &AetherUpdateManifest,
    ) -> Result<PathBuf> {
        UpdateVerifier::verify_payload_checksum(payload, &manifest.sha256_checksum)?;

        std::fs::create_dir_all(&self.staging_dir).map_err(|e| {
            AetherError::Internal(format!("Failed to create staging directory: {e}"))
        })?;

        let staged_file = self
            .staging_dir
            .join(format!("aether-update-{}.bin", manifest.version));
        std::fs::write(&staged_file, payload)
            .map_err(|e| AetherError::Internal(format!("Failed to write staged payload: {e}")))?;

        self.status = StagedUpdateStatus::Staged;
        Ok(staged_file)
    }

    /// Applies a staged update atomically: backs up the currently installed binary,
    /// swaps in the new staged binary, and rolls back if an error occurs.
    pub fn apply_staged_update(&mut self, target_filename: &str, staged_file: &Path) -> Result<()> {
        if self.status != StagedUpdateStatus::Staged {
            return Err(AetherError::Internal(
                "Cannot apply update: no payload is in Staged state".into(),
            ));
        }

        self.status = StagedUpdateStatus::Applying;

        let target_path = self.installed_dir.join(target_filename);
        let backup_path = self
            .installed_dir
            .join(format!("{}.rollback_bak", target_filename));

        // Create backup of current installation if it exists
        let had_existing = target_path.exists();
        if had_existing {
            if let Err(e) = std::fs::copy(&target_path, &backup_path) {
                self.status = StagedUpdateStatus::Failed;
                return Err(AetherError::Internal(format!(
                    "Failed to create rollback backup: {e}"
                )));
            }
        }

        // Deploy new binary from staged file
        if let Err(e) = std::fs::copy(staged_file, &target_path) {
            // Rollback immediately
            if had_existing {
                let _ = std::fs::copy(&backup_path, &target_path);
                let _ = std::fs::remove_file(&backup_path);
            }
            self.status = StagedUpdateStatus::RolledBack;
            return Err(AetherError::Internal(format!(
                "Deployment failed, rolled back to original: {e}"
            )));
        }

        // Clean up backup file
        if had_existing {
            let _ = std::fs::remove_file(&backup_path);
        }

        self.status = StagedUpdateStatus::Completed;
        Ok(())
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
            target_os: None,
            target_arch: None,
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

    #[test]
    fn test_staged_update_lifecycle_and_rollback() {
        let temp_base =
            std::env::temp_dir().join(format!("aether_update_test_{}", uuid::Uuid::new_v4()));
        let staging_dir = temp_base.join("staging");
        let installed_dir = temp_base.join("installed");
        std::fs::create_dir_all(&installed_dir).unwrap();

        // 1. Existing installed binary v0.1.0
        let target_binary = "AetherIDE.exe";
        let target_path = installed_dir.join(target_binary);
        std::fs::write(&target_path, b"AetherIDE Binary v0.1.0").unwrap();

        let mut applicator = StagedUpdateApplicator::new(&staging_dir, &installed_dir);
        assert_eq!(applicator.status(), StagedUpdateStatus::Idle);

        // 2. Prepare v0.2.0 update payload
        let new_payload = b"AetherIDE Binary v0.2.0 Next Generation";
        let mut hasher = Sha256::new();
        hasher.update(new_payload);
        let checksum = format!("{:x}", hasher.finalize());

        let manifest = AetherUpdateManifest {
            version: "0.2.0".into(),
            channel: "beta".into(),
            release_notes: "Commercial Beta v0.2.0".into(),
            download_url: "https://releases.aetheride.dev/v0.2.0/AetherIDE.exe".into(),
            sha256_checksum: checksum,
            signature_base64: "dGVzdC1zaWc=".into(),
            min_os_version: None,
            published_at: Utc::now(),
            target_os: None,
            target_arch: None,
        };

        // 3. Stage the verified update
        let staged_file = applicator.stage_update(new_payload, &manifest).unwrap();
        assert_eq!(applicator.status(), StagedUpdateStatus::Staged);
        assert!(staged_file.exists());

        // 4. Apply the staged update
        applicator
            .apply_staged_update(target_binary, &staged_file)
            .unwrap();
        assert_eq!(applicator.status(), StagedUpdateStatus::Completed);

        // Verify target binary was upgraded
        let upgraded_content = std::fs::read(&target_path).unwrap();
        assert_eq!(upgraded_content, new_payload);

        // Verify rollback backup was cleaned up
        let backup_path = installed_dir.join(format!("{}.rollback_bak", target_binary));
        assert!(!backup_path.exists());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_base);
    }

    #[test]
    fn test_update_manifest_digital_signature_and_platform_binding() {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;
        use ring::rand::SystemRandom;
        use ring::signature::{Ed25519KeyPair, KeyPair};

        let rng = SystemRandom::new();
        let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let keypair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).unwrap();
        let root_pubkey_b64 = BASE64.encode(keypair.public_key().as_ref());

        let mut manifest = AetherUpdateManifest {
            version: "0.2.0".into(),
            channel: "beta".into(),
            release_notes: "Sprint 08 Hardened Manifest".into(),
            download_url: "https://releases.aetheride.dev/v0.2.0/aetheride-windows-x64.zip".into(),
            sha256_checksum: "a".repeat(64),
            signature_base64: String::new(),
            min_os_version: None,
            published_at: Utc::now(),
            target_os: Some(std::env::consts::OS.to_string()),
            target_arch: Some(std::env::consts::ARCH.to_string()),
        };

        // 1. Sign canonical payload
        let canonical = UpdateVerifier::canonical_manifest_payload(&manifest);
        let sig = keypair.sign(canonical.as_bytes());
        manifest.signature_base64 = BASE64.encode(sig.as_ref());

        // 2. Successful verification
        assert!(UpdateVerifier::verify_manifest_signature(&manifest, &root_pubkey_b64).is_ok());

        // 3. Tampered payload is rejected
        let mut tampered = manifest.clone();
        tampered.download_url = "https://evil.com/malware.exe".into();
        assert!(UpdateVerifier::verify_manifest_signature(&tampered, &root_pubkey_b64).is_err());

        // 4. Untrusted / wrong root public key is rejected
        let wrong_pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let wrong_keypair = Ed25519KeyPair::from_pkcs8(wrong_pkcs8.as_ref()).unwrap();
        let wrong_pubkey_b64 = BASE64.encode(wrong_keypair.public_key().as_ref());
        assert!(UpdateVerifier::verify_manifest_signature(&manifest, &wrong_pubkey_b64).is_err());

        // 5. Mismatched OS is rejected
        let mut bad_os = manifest.clone();
        bad_os.target_os = Some("nonexistent_os".into());
        let bad_os_canonical = UpdateVerifier::canonical_manifest_payload(&bad_os);
        let bad_os_sig = keypair.sign(bad_os_canonical.as_bytes());
        bad_os.signature_base64 = BASE64.encode(bad_os_sig.as_ref());
        let err = UpdateVerifier::verify_manifest_signature(&bad_os, &root_pubkey_b64).unwrap_err();
        assert!(err.to_string().contains("Platform mismatch"));

        // 6. Mismatched Arch is rejected
        let mut bad_arch = manifest.clone();
        bad_arch.target_arch = Some("mips64".into());
        let bad_arch_canonical = UpdateVerifier::canonical_manifest_payload(&bad_arch);
        let bad_arch_sig = keypair.sign(bad_arch_canonical.as_bytes());
        bad_arch.signature_base64 = BASE64.encode(bad_arch_sig.as_ref());
        let err =
            UpdateVerifier::verify_manifest_signature(&bad_arch, &root_pubkey_b64).unwrap_err();
        assert!(err.to_string().contains("Architecture mismatch"));
    }
}
