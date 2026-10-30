use ring::aead::{
    Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey, CHACHA20_POLY1305,
    NONCE_LEN,
};
use ring::rand::{SecureRandom, SystemRandom};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyncCryptoError {
    #[error("Authentication failed: ciphertext has been tampered with or key is invalid")]
    AuthenticationFailed,
    #[error("Replay attack detected: sequence counter {0} is less than or equal to current sequence {1}")]
    ReplayDetected(u64, u64),
    #[error("Malformed frame: buffer too short ({0} bytes)")]
    MalformedFrame(usize),
    #[error("RNG error")]
    RngError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncRole {
    Desktop,
    Companion,
}

impl SyncRole {
    fn send_salt(&self) -> [u8; 4] {
        match self {
            SyncRole::Desktop => [b'D', b'S', b'K', 0x01],
            SyncRole::Companion => [b'M', b'O', b'B', 0x01],
        }
    }

    fn recv_salt(&self) -> [u8; 4] {
        match self {
            SyncRole::Desktop => [b'M', b'O', b'B', 0x01],
            SyncRole::Companion => [b'D', b'S', b'K', 0x01],
        }
    }
}

pub fn generate_psk() -> Result<[u8; 32], SyncCryptoError> {
    let rng = SystemRandom::new();
    let mut key = [0u8; 32];
    rng.fill(&mut key).map_err(|_| SyncCryptoError::RngError)?;
    Ok(key)
}

struct FixedNonce(Option<[u8; NONCE_LEN]>);

impl NonceSequence for FixedNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0
            .take()
            .ok_or(ring::error::Unspecified)
            .and_then(|bytes| Nonce::try_assume_unique_for_key(&bytes))
    }
}

fn build_nonce(salt: [u8; 4], seq: u64) -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    nonce[..4].copy_from_slice(&salt);
    nonce[4..12].copy_from_slice(&seq.to_be_bytes());
    nonce
}

pub struct AeadChannel {
    psk: [u8; 32],
    role: SyncRole,
    send_seq: u64,
    recv_seq: u64,
}

impl AeadChannel {
    pub fn new(psk: [u8; 32], role: SyncRole) -> Self {
        Self {
            psk,
            role,
            send_seq: 1,
            recv_seq: 0,
        }
    }

    /// Encrypts plaintext into a self-contained framed message:
    /// `[sequence: 8 bytes][payload_len: 4 bytes][ciphertext + tag: payload_len bytes]`
    pub fn encrypt_frame(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, SyncCryptoError> {
        let seq = self.send_seq;
        self.send_seq += 1;

        let nonce_bytes = build_nonce(self.role.send_salt(), seq);
        let unbound_key = UnboundKey::new(&CHACHA20_POLY1305, &self.psk)
            .map_err(|_| SyncCryptoError::AuthenticationFailed)?;
        let mut sealing_key = SealingKey::new(unbound_key, FixedNonce(Some(nonce_bytes)));

        let mut in_out = plaintext.to_vec();

        // Associated data includes sequence number and original payload length
        let seq_bytes = seq.to_be_bytes();
        let len_bytes = (plaintext.len() as u32).to_be_bytes();
        let mut aad_bytes = Vec::with_capacity(12);
        aad_bytes.extend_from_slice(&seq_bytes);
        aad_bytes.extend_from_slice(&len_bytes);

        sealing_key
            .seal_in_place_append_tag(Aad::from(&aad_bytes), &mut in_out)
            .map_err(|_| SyncCryptoError::AuthenticationFailed)?;

        let mut frame = Vec::with_capacity(12 + in_out.len());
        frame.extend_from_slice(&seq_bytes);
        frame.extend_from_slice(&(in_out.len() as u32).to_be_bytes());
        frame.extend_from_slice(&in_out);

        Ok(frame)
    }

    /// Decrypts and verifies an incoming framed message.
    pub fn decrypt_frame(&mut self, frame: &[u8]) -> Result<Vec<u8>, SyncCryptoError> {
        if frame.len() < 12 + 16 {
            return Err(SyncCryptoError::MalformedFrame(frame.len()));
        }

        let seq = u64::from_be_bytes(frame[..8].try_into().unwrap());
        let payload_len = u32::from_be_bytes(frame[8..12].try_into().unwrap()) as usize;

        if frame.len() < 12 + payload_len {
            return Err(SyncCryptoError::MalformedFrame(frame.len()));
        }

        // Monotonic sequence verification: reject duplicates and replayed old packets
        if seq <= self.recv_seq {
            return Err(SyncCryptoError::ReplayDetected(seq, self.recv_seq));
        }

        let nonce_bytes = build_nonce(self.role.recv_salt(), seq);
        let unbound_key = UnboundKey::new(&CHACHA20_POLY1305, &self.psk)
            .map_err(|_| SyncCryptoError::AuthenticationFailed)?;
        let mut opening_key = OpeningKey::new(unbound_key, FixedNonce(Some(nonce_bytes)));

        let mut in_out = frame[12..12 + payload_len].to_vec();

        let seq_bytes = seq.to_be_bytes();
        let len_bytes = (payload_len as u32 - 16).to_be_bytes();
        let mut aad_bytes = Vec::with_capacity(12);
        aad_bytes.extend_from_slice(&seq_bytes);
        aad_bytes.extend_from_slice(&len_bytes);

        let plaintext = opening_key
            .open_in_place(Aad::from(&aad_bytes), &mut in_out)
            .map_err(|_| SyncCryptoError::AuthenticationFailed)?;

        self.recv_seq = seq;
        Ok(plaintext.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aead_encrypt_decrypt_roundtrip() {
        let psk = generate_psk().unwrap();
        let mut desktop = AeadChannel::new(psk, SyncRole::Desktop);
        let mut companion = AeadChannel::new(psk, SyncRole::Companion);

        let secret_message = b"pub fn main() { println!(\"Encrypted Aether Code\"); }";
        let frame = desktop.encrypt_frame(secret_message).unwrap();

        let decrypted = companion.decrypt_frame(&frame).unwrap();
        assert_eq!(decrypted, secret_message);
    }

    #[test]
    fn test_aead_rejects_tampering() {
        let psk = generate_psk().unwrap();
        let mut desktop = AeadChannel::new(psk, SyncRole::Desktop);
        let mut companion = AeadChannel::new(psk, SyncRole::Companion);

        let mut frame = desktop.encrypt_frame(b"sensitive source code").unwrap();
        // Tamper with last ciphertext byte
        let last = frame.len() - 1;
        frame[last] ^= 0xFF;

        let res = companion.decrypt_frame(&frame);
        assert_eq!(res, Err(SyncCryptoError::AuthenticationFailed));
    }

    #[test]
    fn test_aead_rejects_replay_attack() {
        let psk = generate_psk().unwrap();
        let mut desktop = AeadChannel::new(psk, SyncRole::Desktop);
        let mut companion = AeadChannel::new(psk, SyncRole::Companion);

        let frame1 = desktop.encrypt_frame(b"transaction 1").unwrap();
        let _ = companion.decrypt_frame(&frame1).unwrap();

        // Replaying frame1 should fail
        let res = companion.decrypt_frame(&frame1);
        assert!(matches!(res, Err(SyncCryptoError::ReplayDetected(1, 1))));
    }
}
