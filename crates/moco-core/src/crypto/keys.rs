//! Key material types. All of them zeroize on drop and never print their contents.

use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::random;

pub const KEY_LEN: usize = 32;

/// A 256-bit symmetric key.
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct SymmetricKey([u8; KEY_LEN]);

impl SymmetricKey {
    pub fn generate() -> Self {
        Self(random::bytes::<KEY_LEN>())
    }

    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        let arr: [u8; KEY_LEN] = bytes.try_into().ok()?;
        Some(Self(arr))
    }

    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for SymmetricKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SymmetricKey(***)")
    }
}

/// A secret string (passwords, codes) whose `Debug` output is redacted.
#[derive(Clone, Zeroize, ZeroizeOnDrop, Default, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(***)")
    }
}

impl serde::Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for SecretString {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(SecretString)
    }
}
