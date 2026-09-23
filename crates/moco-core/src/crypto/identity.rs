//! Account identity keys, generated at account creation so every account is ready for
//! authenticated sharing (security review M1):
//! - X25519 — receives vault keys (HPKE, RFC 9180, once sharing ships);
//! - Ed25519 — signs what the account sends (shares, vault-log entries).
//! Private halves are stored encrypted under the Account Key.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fmt;
use x25519_dalek::{PublicKey as XPublic, StaticSecret};
use zeroize::Zeroizing;

use crate::error::{CoreError, Result};

pub struct IdentityKeys {
    x25519: StaticSecret,
    ed25519: SigningKey,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicIdentity {
    #[serde(with = "crate::util::b64")]
    pub x25519: Vec<u8>,
    #[serde(with = "crate::util::b64")]
    pub ed25519: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    #[serde(with = "crate::util::b64")]
    x25519: Vec<u8>,
    #[serde(with = "crate::util::b64")]
    ed25519: Vec<u8>,
}

impl IdentityKeys {
    pub fn generate() -> Self {
        Self { x25519: StaticSecret::random_from_rng(OsRng), ed25519: SigningKey::generate(&mut OsRng) }
    }

    pub fn public(&self) -> PublicIdentity {
        PublicIdentity {
            x25519: XPublic::from(&self.x25519).as_bytes().to_vec(),
            ed25519: self.ed25519.verifying_key().as_bytes().to_vec(),
        }
    }

    pub fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        let stored = Stored { x25519: self.x25519.to_bytes().to_vec(), ed25519: self.ed25519.to_bytes().to_vec() };
        Zeroizing::new(serde_json::to_vec(&stored).expect("serializes"))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let stored: Stored = serde_json::from_slice(bytes).map_err(|_| CoreError::Integrity)?;
        let x: [u8; 32] = stored.x25519.as_slice().try_into().map_err(|_| CoreError::Integrity)?;
        let e: [u8; 32] = stored.ed25519.as_slice().try_into().map_err(|_| CoreError::Integrity)?;
        Ok(Self { x25519: StaticSecret::from(x), ed25519: SigningKey::from_bytes(&e) })
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.ed25519.sign(message).to_bytes()
    }
}

impl fmt::Debug for IdentityKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IdentityKeys(***)")
    }
}

/// Strict Ed25519 verification (rejects malleable/small-order edge cases).
pub fn verify(public_ed25519: &[u8], message: &[u8], signature: &[u8]) -> Result<()> {
    let pk: [u8; 32] = public_ed25519.try_into().map_err(|_| CoreError::Integrity)?;
    let vk = VerifyingKey::from_bytes(&pk).map_err(|_| CoreError::Integrity)?;
    let sig = Signature::from_slice(signature).map_err(|_| CoreError::Integrity)?;
    vk.verify_strict(message, &sig).map_err(|_| CoreError::Integrity)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_signatures() {
        let id = IdentityKeys::generate();
        let restored = IdentityKeys::from_bytes(&id.to_bytes()).unwrap();
        assert_eq!(restored.public(), id.public());
        let sig = id.sign(b"moco");
        verify(&id.public().ed25519, b"moco", &sig).unwrap();
        assert!(verify(&id.public().ed25519, b"outra", &sig).is_err());
    }
}
