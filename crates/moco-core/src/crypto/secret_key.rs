//! The Secret Key ("Chave Secreta") and the Recovery Code ("Código de Recuperação").
//!
//! Both are high-entropy random values meant to be written down, so they use Crockford
//! Base32 with a checksum that catches typos before any expensive crypto runs.

use sha2::{Digest, Sha256};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::random;
use crate::error::{CoreError, Result};
use crate::util::crockford;

const SK_PREFIX: &str = "M1";
const SK_SECRET_LEN: usize = 16; // 128 bits
const SK_TOTAL_LEN: usize = SK_SECRET_LEN + 1; // + checksum byte → 28 symbols (4 pad bits)
const SK_SYMBOLS: usize = 28;

const RC_SECRET_LEN: usize = 18; // 144 bits
const RC_TOTAL_LEN: usize = RC_SECRET_LEN + 2; // + 16-bit checksum → 32 symbols
const RC_SYMBOLS: usize = 32;

fn checksum(label: &[u8], data: &[u8], n: usize) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(label);
    h.update(data);
    h.finalize()[..n].to_vec()
}

fn group(symbols: &str, sizes: &[usize]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    for &s in sizes {
        out.push(&symbols[i..i + s]);
        i += s;
    }
    out.join("-")
}

/// 128-bit device-generated secret mixed into key derivation (2SKD). Never leaves the
/// user's devices; printed on the Emergency Kit.
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct SecretKey([u8; SK_SECRET_LEN]);

impl SecretKey {
    pub fn generate() -> Self {
        Self(random::bytes::<SK_SECRET_LEN>())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        Some(Self(bytes.try_into().ok()?))
    }

    /// `M1-XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXX`
    pub fn to_display(&self) -> Zeroizing<String> {
        let mut payload = Zeroizing::new(Vec::with_capacity(SK_TOTAL_LEN));
        payload.extend_from_slice(&self.0);
        payload.extend_from_slice(&checksum(b"moco/secret-key/v1", &self.0, 1));
        let symbols = Zeroizing::new(crockford::encode(&payload));
        Zeroizing::new(format!("{SK_PREFIX}-{}", group(&symbols, &[6, 6, 6, 6, 4])))
    }

    /// Lenient parser: ignores case, spaces and dashes; maps O→0 and I/L→1.
    pub fn parse(input: &str) -> Result<Self> {
        let cleaned = Zeroizing::new(crockford::clean(input));
        let body = cleaned
            .strip_prefix(SK_PREFIX)
            .ok_or_else(|| CoreError::Invalid("a Chave Secreta começa com M1".into()))?;
        if body.len() != SK_SYMBOLS {
            return Err(CoreError::Invalid("a Chave Secreta está incompleta".into()));
        }
        let bytes = Zeroizing::new(
            crockford::decode(body, SK_TOTAL_LEN)
                .ok_or_else(|| CoreError::Invalid("a Chave Secreta tem caracteres inválidos".into()))?,
        );
        let (secret, check) = bytes.split_at(SK_SECRET_LEN);
        if checksum(b"moco/secret-key/v1", secret, 1) != check {
            return Err(CoreError::Invalid("a Chave Secreta não confere — confira cada caractere".into()));
        }
        Ok(Self(secret.try_into().expect("length checked")))
    }

    /// Short, non-secret hint shown in the UI ("termina em …") — the last group only.
    pub fn hint(&self) -> String {
        let d = self.to_display();
        d.rsplit('-').next().unwrap_or_default().to_string()
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(***)")
    }
}

/// 144-bit recovery code that wraps a copy of the Account Key.
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct RecoveryCode([u8; RC_SECRET_LEN]);

impl RecoveryCode {
    pub fn generate() -> Self {
        Self(random::bytes::<RC_SECRET_LEN>())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Eight groups of four: `XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`.
    pub fn to_display(&self) -> Zeroizing<String> {
        let mut payload = Zeroizing::new(Vec::with_capacity(RC_TOTAL_LEN));
        payload.extend_from_slice(&self.0);
        payload.extend_from_slice(&checksum(b"moco/recovery-code/v1", &self.0, 2));
        let symbols = Zeroizing::new(crockford::encode(&payload));
        Zeroizing::new(group(&symbols, &[4; 8]))
    }

    pub fn parse(input: &str) -> Result<Self> {
        let cleaned = Zeroizing::new(crockford::clean(input));
        if cleaned.len() != RC_SYMBOLS {
            return Err(CoreError::Invalid("o código de recuperação tem 32 caracteres".into()));
        }
        let bytes = Zeroizing::new(
            crockford::decode(&cleaned, RC_TOTAL_LEN)
                .ok_or_else(|| CoreError::Invalid("o código tem caracteres inválidos".into()))?,
        );
        let (secret, check) = bytes.split_at(RC_SECRET_LEN);
        if checksum(b"moco/recovery-code/v1", secret, 2) != check {
            return Err(CoreError::Invalid("o código não confere — confira cada caractere".into()));
        }
        Ok(Self(secret.try_into().expect("length checked")))
    }
}

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_key_roundtrip_and_format() {
        let sk = SecretKey::generate();
        let shown = sk.to_display();
        assert!(shown.starts_with("M1-"));
        assert_eq!(shown.len(), 3 + 28 + 4);
        assert_eq!(SecretKey::parse(&shown).unwrap(), sk);
        // Lenient input: lowercase, spaces instead of dashes.
        let sloppy = shown.to_lowercase().replace('-', " ");
        assert_eq!(SecretKey::parse(&sloppy).unwrap(), sk);
    }

    #[test]
    fn secret_key_detects_typos() {
        let sk = SecretKey::generate();
        let shown = sk.to_display().to_string();
        let mut rejected = 0;
        let alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        for pos in 3..shown.len() {
            let c = shown.as_bytes()[pos] as char;
            if c == '-' {
                continue;
            }
            let replacement = alphabet.chars().find(|&a| a != c).unwrap();
            let mut typo = shown.clone();
            typo.replace_range(pos..pos + 1, &replacement.to_string());
            if SecretKey::parse(&typo).is_err() {
                rejected += 1;
            }
        }
        // 1-byte checksum + 4 zero pad bits: virtually every single-symbol typo is caught.
        assert!(rejected >= 26, "only {rejected} typos caught");
    }

    #[test]
    fn recovery_code_roundtrip() {
        let rc = RecoveryCode::generate();
        let shown = rc.to_display();
        assert_eq!(shown.split('-').count(), 8);
        assert_eq!(RecoveryCode::parse(&shown).unwrap(), rc);
        let mut typo = shown.to_string();
        let first = typo.remove(0);
        typo.insert(0, if first == 'A' { 'B' } else { 'A' });
        assert!(RecoveryCode::parse(&typo).is_err());
    }

    #[test]
    fn debug_is_redacted() {
        assert_eq!(format!("{:?}", SecretKey::generate()), "SecretKey(***)");
        assert_eq!(format!("{:?}", RecoveryCode::generate()), "RecoveryCode(***)");
    }
}
