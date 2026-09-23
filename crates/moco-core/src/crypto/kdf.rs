//! Password and key derivation.

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use super::keys::SymmetricKey;
use super::random;
use crate::error::{CoreError, Result};

pub const SALT_LEN: usize = 16;

/// Argon2id parameters, stored alongside the salt so they can be raised later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    pub algorithm: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    #[serde(with = "crate::util::b64")]
    pub salt: Vec<u8>,
}

impl KdfParams {
    /// Current defaults: Argon2id, 64 MiB, 3 passes, 4 lanes (DECISIONS.md D-003).
    pub fn recommended() -> Self {
        Self {
            algorithm: "argon2id".into(),
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 4,
            salt: random::bytes::<SALT_LEN>().to_vec(),
        }
    }

    /// Rejects parameters that are too weak (downgrade attempts) or absurdly expensive
    /// (denial of service through a crafted vault file).
    pub fn validate(&self) -> Result<()> {
        if self.algorithm != "argon2id" {
            return Err(CoreError::Unsupported(format!("kdf {}", self.algorithm)));
        }
        let ok = (19 * 1024..=2 * 1024 * 1024).contains(&self.memory_kib)
            && (1..=64).contains(&self.iterations)
            && (1..=16).contains(&self.parallelism)
            && self.salt.len() >= SALT_LEN;
        if ok {
            Ok(())
        } else {
            Err(CoreError::Invalid("parâmetros de derivação fora dos limites seguros".into()))
        }
    }

    #[cfg(test)]
    pub fn fast_for_tests() -> Self {
        Self { memory_kib: 19 * 1024, iterations: 1, parallelism: 1, ..Self::recommended() }
    }
}

/// Canonical form of a password before hashing. NFKD makes "ó" typed as one code point
/// or as "o" + combining accent produce the same key — common with Portuguese keyboards
/// across platforms.
pub fn normalize_password(password: &str) -> Zeroizing<String> {
    Zeroizing::new(password.nfkd().collect())
}

/// Argon2id(NFKD(password), salt) → 32 bytes.
pub fn derive_password_key(password: &str, params: &KdfParams) -> Result<Zeroizing<[u8; 32]>> {
    params.validate()?;
    let p = Params::new(params.memory_kib, params.iterations, params.parallelism, Some(32))
        .map_err(|e| CoreError::Invalid(format!("argon2: {e}")))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let normalized = normalize_password(password);
    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(normalized.as_bytes(), &params.salt, out.as_mut())
        .map_err(|e| CoreError::Invalid(format!("argon2: {e}")))?;
    Ok(out)
}

/// HKDF-SHA256 expand into a 256-bit key with a domain-separating label.
pub fn hkdf_key(ikm: &[u8], salt: Option<&[u8]>, info: &str) -> SymmetricKey {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut okm = [0u8; 32];
    hk.expand(info.as_bytes(), &mut okm).expect("32 bytes is a valid HKDF output length");
    let key = SymmetricKey::from_bytes(okm);
    zeroize::Zeroize::zeroize(&mut okm);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2id_known_answer() {
        // RFC 9106 §5.3 (Argon2id) — validates parameters are wired correctly.
        let params = argon2::ParamsBuilder::new()
            .m_cost(32)
            .t_cost(3)
            .p_cost(4)
            .data(argon2::AssociatedData::new(&[0x04; 12]).unwrap())
            .output_len(32)
            .build()
            .unwrap();
        let argon = Argon2::new_with_secret(&[0x03; 8], Algorithm::Argon2id, Version::V0x13, params).unwrap();
        let mut out = [0u8; 32];
        argon.hash_password_into(&[0x01; 32], &[0x02; 16], &mut out).unwrap();
        assert_eq!(
            hex::encode(out),
            "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659"
        );
    }

    #[test]
    fn nfkd_makes_accents_equivalent() {
        let composed = "p\u{00F3}ssego";
        let decomposed = "po\u{0301}ssego";
        let params = KdfParams::fast_for_tests();
        assert_eq!(
            *derive_password_key(composed, &params).unwrap(),
            *derive_password_key(decomposed, &params).unwrap()
        );
        assert_ne!(
            *derive_password_key("possego", &params).unwrap(),
            *derive_password_key(composed, &params).unwrap()
        );
    }

    #[test]
    fn rejects_downgraded_params() {
        let mut p = KdfParams::recommended();
        p.memory_kib = 1024;
        assert!(p.validate().is_err());
        let mut p = KdfParams::recommended();
        p.algorithm = "pbkdf2".into();
        assert!(p.validate().is_err());
        assert!(KdfParams::recommended().validate().is_ok());
    }

    #[test]
    fn hkdf_labels_separate_keys() {
        let a = hkdf_key(b"ikm", Some(b"salt"), "moco/v1/unlock");
        let b = hkdf_key(b"ikm", Some(b"salt"), "moco/v1/auth");
        assert_ne!(a, b);
    }
}
