//! Self-describing, key-committing authenticated encryption envelope.
//!
//! Every ciphertext carries an authenticated header that says exactly what it is and
//! where it belongs. The associated data *is* the header, so a ciphertext cannot be
//! moved to another item, vault, account or purpose, nor re-labelled with another
//! version, without failing authentication. Sync columns (server sequence, timestamps)
//! are never bound — they are cursors, not identity (security review M3).
//!
//! ```text
//! off len
//! 0    1  format        = 0x01
//! 1    1  suite         = 0x01  XChaCha20-Poly1305 + HKDF-SHA256 key commitment
//! 2    1  purpose       (see [`Purpose`])
//! 3    1  flags         bit0 = plaintext padded to a size bucket
//! 4   16  account_id
//! 20  16  container_id  (vault_id, device_id or account_id)
//! 36  16  object_id     (item_id, vault_id…)
//! 52  16  key_id        id of the key that sealed this envelope
//! 68   4  key_gen       u32 BE — generation of that key (rotation)
//! 72   8  version       u64 BE — object version (client: max_seen + 1)
//! 80  16  write_id      random; shared by all blobs of one logical write
//! 96  24  nonce
//! 120 32  commit
//! 152 ..  ciphertext || Poly1305 tag (16)
//! AD = bytes[0..96]
//! ```
//! Per message: `enc_key || commit = HKDF-SHA256(ikm = key, salt = nonce,
//! info = "moco/v1/aead" || header[0..4], L = 64)`. XChaCha20-Poly1305 is not
//! key-committing on its own; checking `commit` in constant time first makes each
//! ciphertext valid under exactly one key.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::keys::SymmetricKey;
use super::random;
use crate::error::{CoreError, Result};

pub const FORMAT: u8 = 0x01;
pub const SUITE_XCHACHA_COMMIT: u8 = 0x01;
const AD_LEN: usize = 96;
const NONCE_LEN: usize = 24;
const COMMIT_LEN: usize = 32;
const TAG_LEN: usize = 16;
const PREFIX_LEN: usize = AD_LEN + NONCE_LEN + COMMIT_LEN;
const FLAG_PADDED: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Purpose {
    AkByPassword = 0x01,
    AkByRecovery = 0x02,
    AkByDevice = 0x03,
    VaultKey = 0x04,
    ItemKey = 0x05,
    ItemOverview = 0x06,
    ItemDetails = 0x07,
    Tombstone = 0x08,
    AttachmentMeta = 0x09,
    PrivateKeys = 0x0A,
    AccountState = 0x0B,
    VaultAttrs = 0x0C,
    LocalState = 0x0D,
    Export = 0x0E,
    Attachment = 0x0F,
    AuthKey = 0x10,
}

impl Purpose {
    fn from_u8(v: u8) -> Option<Self> {
        use Purpose::*;
        Some(match v {
            0x01 => AkByPassword,
            0x02 => AkByRecovery,
            0x03 => AkByDevice,
            0x04 => VaultKey,
            0x05 => ItemKey,
            0x06 => ItemOverview,
            0x07 => ItemDetails,
            0x08 => Tombstone,
            0x09 => AttachmentMeta,
            0x0A => PrivateKeys,
            0x0B => AccountState,
            0x0C => VaultAttrs,
            0x0D => LocalState,
            0x0E => Export,
            0x0F => Attachment,
            0x10 => AuthKey,
            _ => return None,
        })
    }
}

/// Authenticated header. Build one with [`Header::new`] and the `with_*` helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub purpose: Purpose,
    pub padded: bool,
    pub account_id: Uuid,
    pub container_id: Uuid,
    pub object_id: Uuid,
    pub key_id: Uuid,
    pub key_gen: u32,
    pub version: u64,
    pub write_id: [u8; 16],
}

impl Header {
    pub fn new(purpose: Purpose, account_id: Uuid) -> Self {
        Self {
            purpose,
            padded: false,
            account_id,
            container_id: Uuid::nil(),
            object_id: Uuid::nil(),
            key_id: Uuid::nil(),
            key_gen: 0,
            version: 0,
            write_id: random::bytes::<16>(),
        }
    }
    pub fn container(mut self, id: Uuid) -> Self {
        self.container_id = id;
        self
    }
    pub fn object(mut self, id: Uuid) -> Self {
        self.object_id = id;
        self
    }
    pub fn key(mut self, id: Uuid, generation: u32) -> Self {
        self.key_id = id;
        self.key_gen = generation;
        self
    }
    pub fn version(mut self, v: u64) -> Self {
        self.version = v;
        self
    }
    pub fn write(mut self, write_id: [u8; 16]) -> Self {
        self.write_id = write_id;
        self
    }
    pub fn padded(mut self) -> Self {
        self.padded = true;
        self
    }

    fn encode(&self) -> [u8; AD_LEN] {
        let mut h = [0u8; AD_LEN];
        h[0] = FORMAT;
        h[1] = SUITE_XCHACHA_COMMIT;
        h[2] = self.purpose as u8;
        h[3] = if self.padded { FLAG_PADDED } else { 0 };
        h[4..20].copy_from_slice(self.account_id.as_bytes());
        h[20..36].copy_from_slice(self.container_id.as_bytes());
        h[36..52].copy_from_slice(self.object_id.as_bytes());
        h[52..68].copy_from_slice(self.key_id.as_bytes());
        h[68..72].copy_from_slice(&self.key_gen.to_be_bytes());
        h[72..80].copy_from_slice(&self.version.to_be_bytes());
        h[80..96].copy_from_slice(&self.write_id);
        h
    }

    fn decode(h: &[u8]) -> Result<Self> {
        if h.len() < AD_LEN {
            return Err(CoreError::Integrity);
        }
        if h[0] != FORMAT || h[1] != SUITE_XCHACHA_COMMIT {
            return Err(CoreError::Unsupported(format!("envelope format {} suite {}", h[0], h[1])));
        }
        let purpose = Purpose::from_u8(h[2]).ok_or(CoreError::Integrity)?;
        if h[3] & !FLAG_PADDED != 0 {
            return Err(CoreError::Unsupported("envelope flags".into()));
        }
        let uuid = |r: std::ops::Range<usize>| Uuid::from_slice(&h[r]).map_err(|_| CoreError::Integrity);
        Ok(Self {
            purpose,
            padded: h[3] & FLAG_PADDED != 0,
            account_id: uuid(4..20)?,
            container_id: uuid(20..36)?,
            object_id: uuid(36..52)?,
            key_id: uuid(52..68)?,
            key_gen: u32::from_be_bytes(h[68..72].try_into().expect("4 bytes")),
            version: u64::from_be_bytes(h[72..80].try_into().expect("8 bytes")),
            write_id: h[80..96].try_into().expect("16 bytes"),
        })
    }
}

/// What the caller expects an envelope to be. Fields left `None` are not checked.
#[derive(Debug, Clone, Default)]
pub struct Expect {
    pub purpose: Option<Purpose>,
    pub account_id: Option<Uuid>,
    pub container_id: Option<Uuid>,
    pub object_id: Option<Uuid>,
    pub key_id: Option<Uuid>,
    pub min_version: Option<u64>,
}

impl Expect {
    pub fn purpose(p: Purpose) -> Self {
        Self { purpose: Some(p), ..Default::default() }
    }
    pub fn account(mut self, id: Uuid) -> Self {
        self.account_id = Some(id);
        self
    }
    pub fn container(mut self, id: Uuid) -> Self {
        self.container_id = Some(id);
        self
    }
    pub fn object(mut self, id: Uuid) -> Self {
        self.object_id = Some(id);
        self
    }
    pub fn key(mut self, id: Uuid) -> Self {
        self.key_id = Some(id);
        self
    }

    fn check(&self, h: &Header) -> Result<()> {
        let ok = self.purpose.is_none_or(|p| p == h.purpose)
            && self.account_id.is_none_or(|v| v == h.account_id)
            && self.container_id.is_none_or(|v| v == h.container_id)
            && self.object_id.is_none_or(|v| v == h.object_id)
            && self.key_id.is_none_or(|v| v == h.key_id)
            && self.min_version.is_none_or(|v| h.version >= v);
        if ok { Ok(()) } else { Err(CoreError::Integrity) }
    }
}

fn derive(key: &SymmetricKey, nonce: &[u8], header4: &[u8]) -> (Zeroizing<[u8; 32]>, [u8; COMMIT_LEN]) {
    let hk = Hkdf::<Sha256>::new(Some(nonce), key.as_bytes());
    let mut info = [0u8; 16];
    info[..12].copy_from_slice(b"moco/v1/aead");
    info[12..].copy_from_slice(header4);
    let mut okm = Zeroizing::new([0u8; 64]);
    hk.expand(&info, okm.as_mut()).expect("64 bytes is a valid HKDF-SHA256 output length");
    let mut enc = Zeroizing::new([0u8; 32]);
    enc.copy_from_slice(&okm[..32]);
    let mut commit = [0u8; COMMIT_LEN];
    commit.copy_from_slice(&okm[32..]);
    (enc, commit)
}

/// Size bucket for padded plaintexts: 256-byte steps up to 4 KiB, then Padmé
/// (leaks at most O(log log n) bits of the length).
fn padded_len(n: usize) -> usize {
    let n = n + 1; // room for the 0x80 marker
    if n <= 4096 {
        return n.div_ceil(256) * 256;
    }
    let e = usize::BITS - 1 - n.leading_zeros(); // floor(log2 n)
    let s = usize::BITS - 1 - (e as usize).leading_zeros() + 1; // floor(log2 e) + 1
    let last_bits = e - s;
    let mask = (1usize << last_bits) - 1;
    (n + mask) & !mask
}

fn pad(data: &[u8]) -> Zeroizing<Vec<u8>> {
    let target = padded_len(data.len());
    let mut v = Zeroizing::new(Vec::with_capacity(target));
    v.extend_from_slice(data);
    v.push(0x80);
    v.resize(target, 0);
    v
}

fn unpad(mut v: Zeroizing<Vec<u8>>) -> Result<Zeroizing<Vec<u8>>> {
    let end = v.iter().rposition(|&b| b != 0).ok_or(CoreError::Integrity)?;
    if v[end] != 0x80 {
        return Err(CoreError::Integrity);
    }
    v.truncate(end);
    Ok(v)
}

/// Encrypts `plaintext` under `key` with the given authenticated header.
pub fn seal(key: &SymmetricKey, header: &Header, plaintext: &[u8]) -> Vec<u8> {
    let nonce = random::bytes::<NONCE_LEN>();
    let ad = header.encode();
    let (enc, commit) = derive(key, &nonce, &ad[..4]);
    let cipher = XChaCha20Poly1305::new(enc.as_ref().into());
    let body = if header.padded { pad(plaintext) } else { Zeroizing::new(plaintext.to_vec()) };
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: &body, aad: &ad })
        .expect("XChaCha20-Poly1305 encryption cannot fail for in-memory buffers");

    let mut out = Vec::with_capacity(PREFIX_LEN + ct.len());
    out.extend_from_slice(&ad);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&commit);
    out.extend_from_slice(&ct);
    out
}

/// Reads the (authenticated-once-opened) header without decrypting. Useful to route an
/// envelope to the right key; never trust it before [`open`] succeeds.
pub fn peek(envelope: &[u8]) -> Result<Header> {
    Header::decode(envelope)
}

/// Decrypts an envelope produced by [`seal`], checking the header against `expect`.
/// Returns [`CoreError::Integrity`] when the key, header or bytes are wrong.
pub fn open(key: &SymmetricKey, envelope: &[u8], expect: &Expect) -> Result<(Header, Zeroizing<Vec<u8>>)> {
    if envelope.len() < PREFIX_LEN + TAG_LEN {
        return Err(CoreError::Integrity);
    }
    let header = Header::decode(&envelope[..AD_LEN])?;
    expect.check(&header)?;
    let ad = &envelope[..AD_LEN];
    let nonce = &envelope[AD_LEN..AD_LEN + NONCE_LEN];
    let commit = &envelope[AD_LEN + NONCE_LEN..PREFIX_LEN];
    let ct = &envelope[PREFIX_LEN..];

    let (enc, expected) = derive(key, nonce, &ad[..4]);
    if !bool::from(expected.ct_eq(commit)) {
        return Err(CoreError::Integrity);
    }
    let cipher = XChaCha20Poly1305::new(enc.as_ref().into());
    let pt = cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad: ad })
        .map(Zeroizing::new)
        .map_err(|_| CoreError::Integrity)?;
    let pt = if header.padded { unpad(pt)? } else { pt };
    Ok((header, pt))
}

/// Wraps (encrypts) one symmetric key with another.
pub fn wrap_key(wrapping: &SymmetricKey, header: &Header, key: &SymmetricKey) -> Vec<u8> {
    seal(wrapping, header, key.as_bytes())
}

pub fn unwrap_key(wrapping: &SymmetricKey, wrapped: &[u8], expect: &Expect) -> Result<(Header, SymmetricKey)> {
    let (h, raw) = open(wrapping, wrapped, expect)?;
    let k = SymmetricKey::from_slice(&raw).ok_or(CoreError::Integrity)?;
    Ok((h, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xchacha_known_answer() {
        // draft-irtf-cfrg-xchacha-03, Appendix A.3.1 — validates our use of the crate.
        let key: [u8; 32] = hex::decode("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f")
            .unwrap()
            .try_into()
            .unwrap();
        let nonce = hex::decode("404142434445464748494a4b4c4d4e4f5051525354555657").unwrap();
        let aad = hex::decode("50515253c0c1c2c3c4c5c6c7").unwrap();
        let pt = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let cipher = XChaCha20Poly1305::new(&key.into());
        let ct = cipher.encrypt(XNonce::from_slice(&nonce), Payload { msg: pt, aad: &aad }).unwrap();
        assert_eq!(hex::encode(&ct[ct.len() - 16..]), "c0875924c1c7987947deafd8780acf49");
        assert_eq!(hex::encode(&ct[..16]), "bd6d179d3e83d43b9576579493c0e939");
    }

    fn hdr() -> Header {
        Header::new(Purpose::ItemDetails, Uuid::from_u128(1))
            .container(Uuid::from_u128(2))
            .object(Uuid::from_u128(3))
            .key(Uuid::from_u128(2), 1)
            .version(7)
    }

    #[test]
    fn roundtrip_and_expectations() {
        let key = SymmetricKey::generate();
        let env = seal(&key, &hdr(), b"segredo");
        let (h, pt) = open(&key, &env, &Expect::purpose(Purpose::ItemDetails).object(Uuid::from_u128(3))).unwrap();
        assert_eq!(&pt[..], b"segredo");
        assert_eq!(h.version, 7);
        assert_eq!(peek(&env).unwrap(), h);

        // Wrong expectations are rejected before any decryption.
        assert!(open(&key, &env, &Expect::purpose(Purpose::ItemOverview)).is_err());
        assert!(open(&key, &env, &Expect::default().object(Uuid::from_u128(4))).is_err());
        let mut e = Expect::default();
        e.min_version = Some(8);
        assert!(open(&key, &env, &e).is_err(), "rollback below high-water mark");
        // Wrong key → rejected at the commitment check.
        assert!(open(&SymmetricKey::generate(), &env, &Expect::default()).is_err());
    }

    #[test]
    fn header_is_authenticated() {
        let key = SymmetricKey::generate();
        let env = seal(&key, &hdr(), b"conteudo importante");
        for i in 0..env.len() {
            let mut bad = env.clone();
            bad[i] ^= 0x01;
            assert!(open(&key, &bad, &Expect::default()).is_err(), "flip at byte {i} was accepted");
        }
        assert!(open(&key, &env[..env.len() - 1], &Expect::default()).is_err());
    }

    #[test]
    fn padding_hides_exact_length() {
        let key = SymmetricKey::generate();
        let a = seal(&key, &hdr().padded(), &[7u8; 10]);
        let b = seal(&key, &hdr().padded(), &[7u8; 200]);
        assert_eq!(a.len(), b.len());
        let (_, pt) = open(&key, &b, &Expect::default()).unwrap();
        assert_eq!(pt.len(), 200);
        // Trailing zeros inside real data survive the unpad.
        let c = seal(&key, &hdr().padded(), &[1, 0, 0, 0]);
        assert_eq!(&open(&key, &c, &Expect::default()).unwrap().1[..], &[1, 0, 0, 0]);
        for n in [0usize, 1, 255, 256, 4095, 4096, 5000, 70_000, 1_000_000] {
            assert!(padded_len(n) > n, "bucket for {n}");
        }
    }

    #[test]
    fn nonces_are_unique() {
        let key = SymmetricKey::generate();
        assert_ne!(seal(&key, &hdr(), b"a"), seal(&key, &hdr(), b"a"));
    }

    #[test]
    fn key_wrapping() {
        let kek = SymmetricKey::generate();
        let k = SymmetricKey::generate();
        let h = Header::new(Purpose::VaultKey, Uuid::from_u128(1));
        let w = wrap_key(&kek, &h, &k);
        assert_eq!(unwrap_key(&kek, &w, &Expect::purpose(Purpose::VaultKey)).unwrap().1, k);
    }
}
