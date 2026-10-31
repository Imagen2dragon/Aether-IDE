use application_core::{AetherError, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};

/// Role-based access control tiers for collaborative workspaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeviceRole {
    Viewer = 0,
    Reviewer = 1,
    Editor = 2,
    Admin = 3,
    Owner = 4,
}

impl DeviceRole {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "owner" => DeviceRole::Owner,
            "admin" => DeviceRole::Admin,
            "editor" => DeviceRole::Editor,
            "reviewer" => DeviceRole::Reviewer,
            _ => DeviceRole::Viewer,
        }
    }

    /// Whether this role is authorized to apply document mutations and CRDT updates.
    pub fn can_write(&self) -> bool {
        matches!(
            self,
            DeviceRole::Editor | DeviceRole::Admin | DeviceRole::Owner
        )
    }

    /// Whether this role is authorized to invite peers, change roles, or destroy workspaces.
    pub fn can_admin(&self) -> bool {
        matches!(self, DeviceRole::Admin | DeviceRole::Owner)
    }

    /// Whether this role can read documents and track peer presence.
    pub fn can_read(&self) -> bool {
        true
    }
}

/// Cryptographic device identity holding an Ed25519 keypair and assigned permissions.
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
    pub role: DeviceRole,
    key_pair: Ed25519KeyPair,
}

impl DeviceIdentity {
    /// Generates a new random Ed25519 keypair for this device.
    pub fn generate(
        device_id: impl Into<String>,
        device_name: impl Into<String>,
        role: DeviceRole,
    ) -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| {
            AetherError::Internal(format!("Failed to generate Ed25519 keypair: {e}"))
        })?;
        let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|e| {
            AetherError::Internal(format!("Failed to parse generated PKCS#8 key: {e}"))
        })?;

        Ok(Self {
            device_id: device_id.into(),
            device_name: device_name.into(),
            role,
            key_pair,
        })
    }

    /// Exports the 32-byte Ed25519 public key as base64 string.
    pub fn public_key_base64(&self) -> String {
        BASE64.encode(self.key_pair.public_key().as_ref())
    }

    /// Signs a message buffer using the device's private Ed25519 key.
    pub fn sign_message(&self, msg: &[u8]) -> String {
        let sig = self.key_pair.sign(msg);
        BASE64.encode(sig.as_ref())
    }

    /// Verifies a digital signature from a remote peer given their public key.
    pub fn verify_signature(
        peer_public_key_base64: &str,
        msg: &[u8],
        signature_base64: &str,
    ) -> Result<()> {
        let pk_bytes = BASE64
            .decode(peer_public_key_base64)
            .map_err(|e| AetherError::Internal(format!("Invalid public key base64: {e}")))?;
        let sig_bytes = BASE64
            .decode(signature_base64)
            .map_err(|e| AetherError::Internal(format!("Invalid signature base64: {e}")))?;

        let peer_verifier = UnparsedPublicKey::new(&ED25519, pk_bytes);
        peer_verifier.verify(msg, &sig_bytes).map_err(|e| {
            AetherError::Internal(format!("Ed25519 signature verification failed: {e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_roles_permissions() {
        assert!(!DeviceRole::Viewer.can_write());
        assert!(!DeviceRole::Reviewer.can_write());
        assert!(DeviceRole::Editor.can_write());
        assert!(DeviceRole::Admin.can_write());
        assert!(DeviceRole::Owner.can_write());

        assert!(!DeviceRole::Editor.can_admin());
        assert!(DeviceRole::Admin.can_admin());
        assert!(DeviceRole::Owner.can_admin());
    }

    #[test]
    fn test_ed25519_sign_and_verify() {
        let alice =
            DeviceIdentity::generate("dev-1", "Alice Workstation", DeviceRole::Editor).unwrap();
        let payload = b"CRDT_UPDATE_OPERATION_HASH_0x123456";

        let signature = alice.sign_message(payload);
        let pubkey = alice.public_key_base64();

        // Valid signature verification
        let res = DeviceIdentity::verify_signature(&pubkey, payload, &signature);
        assert!(res.is_ok());

        // Tampered payload fails verification
        let tampered = b"CRDT_UPDATE_OPERATION_HASH_TAMPERED";
        let res_tampered = DeviceIdentity::verify_signature(&pubkey, tampered, &signature);
        assert!(res_tampered.is_err());
    }
}
