use crate::crypto::generate_psk;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairingPayload {
    pub host: String,
    pub port: u16,
    pub session_id: String,
    pub psk_hex: String,
    pub expires_at: u64,
}

impl PairingPayload {
    pub fn new(
        host: &str,
        port: u16,
        ttl_secs: u64,
    ) -> Result<(Self, [u8; 32]), crate::crypto::SyncCryptoError> {
        let psk = generate_psk()?;
        let psk_hex = hex_encode(&psk);
        let session_id = uuid::Uuid::new_v4().to_string();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let payload = Self {
            host: host.to_string(),
            port,
            session_id,
            psk_hex,
            expires_at: now + ttl_secs,
        };

        Ok((payload, psk))
    }

    pub fn to_pairing_uri(&self) -> String {
        format!(
            "aether://pair?host={}&port={}&session={}&key={}&exp={}",
            self.host, self.port, self.session_id, self.psk_hex, self.expires_at
        )
    }

    pub fn parse_pairing_uri(uri: &str) -> Result<Self, String> {
        if !uri.starts_with("aether://pair?") {
            return Err("Invalid URI scheme: expected aether://pair?".to_string());
        }

        let query = &uri["aether://pair?".len()..];
        let mut host = None;
        let mut port = None;
        let mut session = None;
        let mut key = None;
        let mut exp = None;

        for param in query.split('&') {
            if let Some((k, v)) = param.split_once('=') {
                match k {
                    "host" => host = Some(v.to_string()),
                    "port" => port = v.parse::<u16>().ok(),
                    "session" => session = Some(v.to_string()),
                    "key" => key = Some(v.to_string()),
                    "exp" => exp = v.parse::<u64>().ok(),
                    _ => {}
                }
            }
        }

        match (host, port, session, key, exp) {
            (Some(host), Some(port), Some(session_id), Some(psk_hex), Some(expires_at)) => {
                Ok(Self {
                    host,
                    port,
                    session_id,
                    psk_hex,
                    expires_at,
                })
            }
            _ => Err("Missing required pairing parameters in URI".to_string()),
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now > self.expires_at
    }

    pub fn get_psk_bytes(&self) -> Result<[u8; 32], String> {
        hex_decode(&self.psk_hex)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64 {
        return Err("Hex string must be 64 characters for 32 bytes".to_string());
    }

    let mut out = [0u8; 32];
    for i in 0..32 {
        let byte_str = &hex[i * 2..i * 2 + 2];
        out[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|e| format!("Invalid hex at byte {}: {}", i, e))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_uri_roundtrip() {
        let (pairing, psk) = PairingPayload::new("192.168.1.50", 44999, 300).unwrap();
        assert!(!pairing.is_expired());

        let uri = pairing.to_pairing_uri();
        let parsed = PairingPayload::parse_pairing_uri(&uri).unwrap();

        assert_eq!(pairing, parsed);
        let decoded_psk = parsed.get_psk_bytes().unwrap();
        assert_eq!(decoded_psk, psk);
    }
}
