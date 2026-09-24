//! Vault sharing grants: a vault key sealed to one member and signed by the vault owner.
//!
//! ```text
//! eph = X25519 ephemeral          dh = X25519(eph, member.x25519)
//! wk  = HKDF-SHA256(salt = none, ikm = dh,
//!                   info = "moco/v1/share" ‖ eph_pub ‖ member.x25519)
//! sealed_key = envelope(wk, header{Share, account = owner, container = vault,
//!                                  object = member, key = vault/key_gen}, vault_key)
//! signature  = Ed25519(owner, "moco/v1/share-grant" ‖ owner ‖ vault ‖ member ‖ role
//!                             ‖ key_gen ‖ eph_pub ‖ sealed_key)
//! ```
//! The server stores and relays grants but can't open them (no member private key) and
//! can't forge them (no owner signing key). What it *could* do is hand out a wrong
//! public key; clients pin keys on first use and show a safety number (see `Account`).

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;
use x25519_dalek::{EphemeralSecret, PublicKey as XPublic};
use zeroize::Zeroizing;

use super::aead::{self, Expect, Header, Purpose};
use super::identity::{self, IdentityKeys, PublicIdentity};
use super::keys::SymmetricKey;
use crate::error::{CoreError, Result};

/// What a member may do in a shared vault. Enforced by the server for writes; every
/// member can read (they hold the key).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Editor,
    Reader,
}

impl Role {
    fn byte(self) -> u8 {
        match self {
            Role::Owner => 1,
            Role::Editor => 2,
            Role::Reader => 3,
        }
    }
    pub fn can_write(self) -> bool {
        self != Role::Reader
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShareGrant {
    pub owner_id: Uuid,
    pub vault_id: Uuid,
    pub member_id: Uuid,
    pub role: Role,
    pub key_gen: u32,
    #[serde(with = "crate::util::b64")]
    pub ephemeral: Vec<u8>,
    #[serde(with = "crate::util::b64")]
    pub sealed_key: Vec<u8>,
    #[serde(with = "crate::util::b64")]
    pub signature: Vec<u8>,
}

fn wrapping_key(dh: &[u8; 32], eph_pub: &[u8], member_x: &[u8]) -> SymmetricKey {
    let hk = Hkdf::<Sha256>::new(None, dh);
    let mut info = b"moco/v1/share".to_vec();
    info.extend_from_slice(eph_pub);
    info.extend_from_slice(member_x);
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(&info, okm.as_mut()).expect("valid length");
    SymmetricKey::from_bytes(*okm)
}

fn signed_message(g: &ShareGrant) -> Vec<u8> {
    let mut m = b"moco/v1/share-grant".to_vec();
    m.extend_from_slice(g.owner_id.as_bytes());
    m.extend_from_slice(g.vault_id.as_bytes());
    m.extend_from_slice(g.member_id.as_bytes());
    m.push(g.role.byte());
    m.extend_from_slice(&g.key_gen.to_be_bytes());
    m.extend_from_slice(&(g.ephemeral.len() as u32).to_be_bytes());
    m.extend_from_slice(&g.ephemeral);
    m.extend_from_slice(&g.sealed_key);
    m
}

/// Seals `vault_key` to `member` and signs the grant with the owner's identity.
#[allow(clippy::too_many_arguments)]
pub fn grant(
    owner: &IdentityKeys,
    owner_id: Uuid,
    vault_id: Uuid,
    key_gen: u32,
    vault_key: &SymmetricKey,
    member_id: Uuid,
    member: &PublicIdentity,
    role: Role,
) -> Result<ShareGrant> {
    let member_x: [u8; 32] = member.x25519.as_slice().try_into().map_err(|_| CoreError::Integrity)?;
    let eph = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
    let eph_pub = XPublic::from(&eph).as_bytes().to_vec();
    let dh = eph.diffie_hellman(&XPublic::from(member_x));
    if !dh.was_contributory() {
        return Err(CoreError::Integrity);
    }
    let wk = wrapping_key(dh.as_bytes(), &eph_pub, &member_x);
    let header = Header::new(Purpose::Share, owner_id).container(vault_id).object(member_id).key(vault_id, key_gen).version(key_gen as u64);
    let mut g = ShareGrant {
        owner_id,
        vault_id,
        member_id,
        role,
        key_gen,
        ephemeral: eph_pub,
        sealed_key: aead::wrap_key(&wk, &header, vault_key),
        signature: Vec::new(),
    };
    g.signature = owner.sign(&signed_message(&g)).to_vec();
    Ok(g)
}

/// Checks only the owner's signature (what the server can do: it has no member key).
pub fn verify(g: &ShareGrant, owner_ed25519: &[u8]) -> Result<()> {
    if g.role == Role::Owner {
        return Err(CoreError::Integrity);
    }
    identity::verify(owner_ed25519, &signed_message(g), &g.signature)
}

/// Verifies the owner's signature and opens the vault key with the member's identity.
pub fn open(g: &ShareGrant, owner: &PublicIdentity, me: &IdentityKeys, my_id: Uuid) -> Result<SymmetricKey> {
    if g.member_id != my_id || g.role == Role::Owner {
        return Err(CoreError::Integrity);
    }
    identity::verify(&owner.ed25519, &signed_message(g), &g.signature)?;
    let dh = me.agree(&g.ephemeral)?;
    let my_x = me.public().x25519;
    let wk = wrapping_key(&dh, &g.ephemeral, &my_x);
    let (h, key) = aead::unwrap_key(
        &wk,
        &g.sealed_key,
        &Expect::purpose(Purpose::Share).account(g.owner_id).container(g.vault_id).object(my_id).key(g.vault_id),
    )?;
    if h.version != g.key_gen as u64 || h.key_gen != g.key_gen {
        return Err(CoreError::Integrity);
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (IdentityKeys, IdentityKeys, Uuid, Uuid, Uuid, SymmetricKey) {
        (IdentityKeys::generate(), IdentityKeys::generate(), Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), SymmetricKey::generate())
    }

    #[test]
    fn member_opens_grant() {
        let (owner, member, oid, mid, vid, key) = setup();
        let g = grant(&owner, oid, vid, 3, &key, mid, &member.public(), Role::Editor).unwrap();
        let got = open(&g, &owner.public(), &member, mid).unwrap();
        assert_eq!(got.as_bytes(), key.as_bytes());
    }

    #[test]
    fn tampering_and_wrong_parties_fail() {
        let (owner, member, oid, mid, vid, key) = setup();
        let g = grant(&owner, oid, vid, 1, &key, mid, &member.public(), Role::Reader).unwrap();
        // Role upgrade by the server breaks the signature.
        let mut up = g.clone();
        up.role = Role::Editor;
        assert!(open(&up, &owner.public(), &member, mid).is_err());
        // Moved to another vault.
        let mut moved = g.clone();
        moved.vault_id = Uuid::new_v4();
        assert!(open(&moved, &owner.public(), &member, mid).is_err());
        // Signed by someone else (server substituting the owner key).
        let mallory = IdentityKeys::generate();
        assert!(open(&g, &mallory.public(), &member, mid).is_err());
        // Another member can't open it.
        let other = IdentityKeys::generate();
        assert!(open(&g, &owner.public(), &other, mid).is_err());
        assert!(open(&g, &owner.public(), &member, Uuid::new_v4()).is_err());
    }

    #[test]
    fn fingerprint_is_stable_and_distinct() {
        let a = IdentityKeys::generate().public();
        let b = IdentityKeys::generate().public();
        assert_eq!(a.fingerprint(), a.fingerprint());
        assert_ne!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint().len(), 6 * 5 + 5);
    }
}
