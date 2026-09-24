//! Server-side secret (from `SERVER_SECRET`). Used only for things that are the server's
//! own business: decoy pre-login answers and encrypting 2FA secrets at rest. It cannot
//! decrypt any vault data — the server never has those keys.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

pub struct ServerSecrets {
    root: Zeroizing<Vec<u8>>,
}

impl ServerSecrets {
    pub fn from_env() -> Self {
        let raw = std::env::var("SERVER_SECRET").expect("SERVER_SECRET must be set (32+ random bytes, base64)");
        assert!(raw.len() >= 32, "SERVER_SECRET too short");
        Self { root: Zeroizing::new(raw.into_bytes()) }
    }

    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self { root: Zeroizing::new(b"test-secret-test-secret-test-secret".to_vec()) }
    }

    fn subkey(&self, label: &str) -> Zeroizing<[u8; 32]> {
        let hk = Hkdf::<Sha256>::new(None, &self.root);
        let mut k = Zeroizing::new([0u8; 32]);
        hk.expand(label.as_bytes(), k.as_mut()).expect("len");
        k
    }

    /// Deterministic pseudo-random bytes for a label (decoy pre-login answers).
    pub fn prf(&self, label: &str, input: &[u8], len: usize) -> Vec<u8> {
        let k = self.subkey("moco-server/prf");
        let mut out = Vec::new();
        let mut counter = 0u8;
        while out.len() < len {
            let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(k.as_ref()).expect("key");
            mac.update(label.as_bytes());
            mac.update(&[counter]);
            mac.update(input);
            out.extend_from_slice(&mac.finalize().into_bytes());
            counter += 1;
        }
        out.truncate(len);
        out
    }

    pub fn seal(&self, plaintext: &[u8]) -> Vec<u8> {
        let k = self.subkey("moco-server/at-rest");
        let cipher = XChaCha20Poly1305::new(k.as_ref().into());
        let mut nonce = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let mut out = nonce.to_vec();
        out.extend(cipher.encrypt(XNonce::from_slice(&nonce), plaintext).expect("encrypt"));
        out
    }

    pub fn open(&self, blob: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
        if blob.len() < 24 {
            return None;
        }
        let k = self.subkey("moco-server/at-rest");
        let cipher = XChaCha20Poly1305::new(k.as_ref().into());
        cipher.decrypt(XNonce::from_slice(&blob[..24]), &blob[24..]).ok().map(Zeroizing::new)
    }
}
