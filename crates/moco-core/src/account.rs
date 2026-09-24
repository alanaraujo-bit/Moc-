//! The local account: key hierarchy (DECISIONS.md D-003), unlock/lock, vaults and items.
//!
//! ```text
//! password ─Argon2id─┐
//!                    ├─HKDF-Extract(salt = Secret Key)─ prk ─┬ "moco/v1/unlock"‖acct → MUK ─wraps→ AK
//! Secret Key ────────┘                                       └ "moco/v1/auth-ed25519"‖acct → login key (server)
//! Recovery Code ─HKDF-Extract(salt = Secret Key)─ "moco/v1/recovery-wrap"‖acct ────wraps→ AK
//! Device key (Windows Hello) ──────────────────────────────────────────────────────wraps→ AK
//! AK ─wraps→ vault keys · encrypts identity keys (X25519 + Ed25519) and local state
//! vault key ─encrypts→ item overview / details / tombstones, vault attributes
//! ```
//! Every ciphertext carries an authenticated header (see `crypto::aead`).

use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::aead::{self, Expect, Header, Purpose};
use crate::crypto::identity::{IdentityKeys, PublicIdentity};
use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::{random, RecoveryCode, SecretKey, SymmetricKey};
use crate::error::{CoreError, Result};
use crate::model::{
    now_ms, AttachmentMeta, Details, Field, FieldType, Item, ItemInput, ItemKind, ItemSummary, Overview, PasswordHistoryEntry,
    Timestamp,
};
use crate::store::{HistoryRow, ItemRow, Store, VaultRow};

const ACCOUNT_META: &str = "account";
const LOCAL_STATE_META: &str = "local-state";
const DEVICE_KEY_PREFIX: &str = "device-key:";
const HISTORY_KEEP: usize = 20;
const PASSWORD_HISTORY_KEEP: usize = 30;
pub const TRASH_RETENTION_DAYS: i64 = 30;
pub const MIN_PASSWORD_CHARS: usize = 10;
pub const MAX_ATTACHMENT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AccountRecord {
    format: u32,
    id: Uuid,
    kdf: KdfParams,
    /// Generation of the Account Key (bumped on compromise rotation).
    ak_gen: u32,
    created_at: Timestamp,
    password_changed_at: Timestamp,
    #[serde(with = "crate::util::b64")]
    wrapped_ak_password: Vec<u8>,
    #[serde(default, with = "opt_b64")]
    wrapped_ak_recovery: Option<Vec<u8>>,
    #[serde(default)]
    recovery_created_at: Option<Timestamp>,
    public_identity: PublicIdentity,
    #[serde(with = "crate::util::b64")]
    encrypted_identity: Vec<u8>,
}

mod opt_b64 {
    use data_encoding::BASE64;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(b) => s.serialize_some(&BASE64.encode(b)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let o = Option::<String>::deserialize(d)?;
        o.map(|s| BASE64.decode(s.as_bytes()).map_err(serde::de::Error::custom)).transpose()
    }
}

/// Vault attributes (encrypted at rest).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VaultAttrs {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Named glyph for the vault ("house", "briefcase", "users"…).
    #[serde(default)]
    pub icon: String,
    /// Palette token ("clay", "moss", "ink"…).
    #[serde(default)]
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    pub id: Uuid,
    #[serde(flatten)]
    pub attrs: VaultAttrs,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub item_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ItemVersion {
    pub revision: u64,
    pub saved_at: Timestamp,
    pub item: Item,
}

/// Device-local, encrypted state that never syncs (recents, usage).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocalState {
    #[serde(default)]
    pub usage: HashMap<Uuid, Usage>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub last_used_at: Timestamp,
    pub count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub initialized: bool,
    pub unlocked: bool,
    pub account_id: Option<Uuid>,
    pub created_at: Option<Timestamp>,
    pub password_changed_at: Option<Timestamp>,
    pub recovery_enabled: bool,
    pub recovery_created_at: Option<Timestamp>,
    pub device_keys: Vec<String>,
}

struct VaultState {
    key: SymmetricKey,
    key_gen: u32,
    attrs: VaultAttrs,
    created_at: Timestamp,
    updated_at: Timestamp,
    revision: u64,
}

#[derive(Clone)]
struct CachedItem {
    vault_id: Uuid,
    kind: ItemKind,
    overview: Overview,
    revision: u64,
    created_at: Timestamp,
    updated_at: Timestamp,
}

struct Session {
    record: AccountRecord,
    ak: SymmetricKey,
    identity: IdentityKeys,
    vaults: HashMap<Uuid, VaultState>,
    items: HashMap<Uuid, CachedItem>,
    local: LocalState,
}

pub struct Account {
    store: Store,
    session: Option<Session>,
    /// Raise stored KDF parameters to the current recommendation on unlock.
    pub upgrade_kdf_on_unlock: bool,
}

// ---- key derivation -----------------------------------------------------------------

fn label(prefix: &str, account: &Uuid) -> Vec<u8> {
    let mut v = prefix.as_bytes().to_vec();
    v.extend_from_slice(account.as_bytes());
    v
}

/// MUK from password + Secret Key (2SKD). The raw 16 Secret Key bytes are the HKDF salt.
fn derive_unlock_key(password: &str, secret_key: &SecretKey, params: &KdfParams, account: &Uuid) -> Result<SymmetricKey> {
    let pw = kdf::derive_password_key(password, params)?;
    let hk = Hkdf::<Sha256>::new(Some(secret_key.as_bytes()), pw.as_ref());
    let mut unlock = Zeroizing::new([0u8; 32]);
    hk.expand(&label("moco/v1/unlock", account), unlock.as_mut()).expect("valid length");
    Ok(SymmetricKey::from_bytes(*unlock))
}

/// Recovery wrapping key. Needs the Secret Key too, so a lost recovery sheet alone can't
/// open anything (security review M4).
fn recovery_key(code: &RecoveryCode, secret_key: &SecretKey, account: &Uuid) -> SymmetricKey {
    let hk = Hkdf::<Sha256>::new(Some(secret_key.as_bytes()), code.as_bytes());
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(&label("moco/v1/recovery-wrap", account), okm.as_mut()).expect("valid length");
    SymmetricKey::from_bytes(*okm)
}

/// Stable id for a device credential label (used as the envelope container id).
fn device_uuid(label: &str) -> Uuid {
    let d = Sha256::digest(label.as_bytes());
    Uuid::from_slice(&d[..16]).expect("16 bytes")
}

fn kdf_is_weaker(stored: &KdfParams, policy: &KdfParams) -> bool {
    stored.memory_kib < policy.memory_kib || stored.iterations < policy.iterations
}

// ---- envelope headers ---------------------------------------------------------------

fn ak_wrap_header(purpose: Purpose, rec: &AccountRecord, container: Uuid) -> Header {
    Header::new(purpose, rec.id).container(container).object(rec.id).version(rec.ak_gen as u64)
}

fn item_header(purpose: Purpose, account: Uuid, item: &Item, key_gen: u32, write_id: [u8; 16]) -> Header {
    Header::new(purpose, account)
        .container(item.vault_id)
        .object(item.id)
        .key(item.vault_id, key_gen)
        .version(item.revision)
        .write(write_id)
        .padded()
}

fn item_expect(purpose: Purpose, account: Uuid, row_vault: Uuid, row_item: Uuid) -> Expect {
    Expect::purpose(purpose).account(account).container(row_vault).object(row_item).key(row_vault)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SealedOverview {
    kind: ItemKind,
    #[serde(flatten)]
    overview: Overview,
}

impl Account {
    pub fn new(store: Store) -> Self {
        Self { store, session: None, upgrade_kdf_on_unlock: true }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    fn record(&self) -> Result<Option<AccountRecord>> {
        match self.store.meta_get(ACCOUNT_META)? {
            Some(raw) => Ok(Some(serde_json::from_slice(&raw)?)),
            None => Ok(None),
        }
    }

    fn require_record(&self) -> Result<AccountRecord> {
        self.record()?.ok_or_else(|| CoreError::NotFound("conta".into()))
    }

    pub fn is_initialized(&self) -> Result<bool> {
        Ok(self.store.meta_get(ACCOUNT_META)?.is_some())
    }

    pub fn is_unlocked(&self) -> bool {
        self.session.is_some()
    }

    pub fn status(&self) -> Result<AccountStatus> {
        let rec = self.record()?;
        Ok(AccountStatus {
            initialized: rec.is_some(),
            unlocked: self.session.is_some(),
            account_id: rec.as_ref().map(|r| r.id),
            created_at: rec.as_ref().map(|r| r.created_at),
            password_changed_at: rec.as_ref().map(|r| r.password_changed_at),
            recovery_enabled: rec.as_ref().is_some_and(|r| r.wrapped_ak_recovery.is_some()),
            recovery_created_at: rec.as_ref().and_then(|r| r.recovery_created_at),
            device_keys: self.device_key_labels()?,
        })
    }

    fn session(&self) -> Result<&Session> {
        self.session.as_ref().ok_or(CoreError::Locked)
    }

    fn session_mut(&mut self) -> Result<&mut Session> {
        self.session.as_mut().ok_or(CoreError::Locked)
    }

    fn save_record(&self) -> Result<()> {
        let raw = serde_json::to_vec(&self.session()?.record)?;
        self.store.meta_set(ACCOUNT_META, &raw)
    }

    // ---- lifecycle --------------------------------------------------------------------

    /// Creates a brand-new account with one vault. Returns the recovery code, which must
    /// be shown to the user exactly once.
    pub fn create(&mut self, password: &str, secret_key: &SecretKey, first_vault: VaultAttrs) -> Result<RecoveryCode> {
        self.create_with_params(password, secret_key, first_vault, KdfParams::recommended())
    }

    pub fn create_with_params(
        &mut self,
        password: &str,
        secret_key: &SecretKey,
        first_vault: VaultAttrs,
        kdf_params: KdfParams,
    ) -> Result<RecoveryCode> {
        if self.is_initialized()? {
            return Err(CoreError::Conflict("já existe um Mocó neste computador".into()));
        }
        check_master_password(password)?;
        let id = Uuid::new_v4();
        let muk = derive_unlock_key(password, secret_key, &kdf_params, &id)?;
        let ak = SymmetricKey::generate();
        let identity = IdentityKeys::generate();
        let code = RecoveryCode::generate();
        let now = now_ms();

        let mut record = AccountRecord {
            format: 1,
            id,
            kdf: kdf_params,
            ak_gen: 1,
            created_at: now,
            password_changed_at: now,
            wrapped_ak_password: Vec::new(),
            wrapped_ak_recovery: None,
            recovery_created_at: Some(now),
            public_identity: identity.public(),
            encrypted_identity: Vec::new(),
        };
        record.wrapped_ak_password = aead::wrap_key(&muk, &ak_wrap_header(Purpose::AkByPassword, &record, id), &ak);
        record.wrapped_ak_recovery = Some(aead::wrap_key(
            &recovery_key(&code, secret_key, &id),
            &ak_wrap_header(Purpose::AkByRecovery, &record, id),
            &ak,
        ));
        record.encrypted_identity = aead::seal(
            &ak,
            &Header::new(Purpose::PrivateKeys, id).container(id).object(id).key(id, 1).padded(),
            &identity.to_bytes(),
        );

        self.store.transaction(|s| {
            s.meta_set(ACCOUNT_META, &serde_json::to_vec(&record)?)?;
            s.log_event("account.created", None)
        })?;

        self.session = Some(Session {
            record,
            ak,
            identity,
            vaults: HashMap::new(),
            items: HashMap::new(),
            local: LocalState::default(),
        });
        self.create_vault(first_vault)?;
        Ok(code)
    }

    pub fn unlock(&mut self, password: &str, secret_key: &SecretKey) -> Result<()> {
        let record = self.require_record()?;
        let muk = derive_unlock_key(password, secret_key, &record.kdf, &record.id)?;
        let expect = Expect::purpose(Purpose::AkByPassword).account(record.id).object(record.id);
        let ak = match aead::unwrap_key(&muk, &record.wrapped_ak_password, &expect) {
            Ok((_, k)) => k,
            Err(CoreError::Integrity) => {
                let _ = self.store.log_event("unlock.failed", None);
                return Err(CoreError::BadCredentials);
            }
            Err(e) => return Err(e),
        };
        let weak = kdf_is_weaker(&record.kdf, &KdfParams::recommended());
        self.finish_unlock(record, ak, "unlock.password")?;
        if weak && self.upgrade_kdf_on_unlock {
            self.set_password(password, secret_key)?;
            self.store.log_event("kdf.upgraded", None)?;
        }
        Ok(())
    }

    fn finish_unlock(&mut self, record: AccountRecord, ak: SymmetricKey, event: &str) -> Result<()> {
        let acct = record.id;
        let (_, raw) = aead::open(
            &ak,
            &record.encrypted_identity,
            &Expect::purpose(Purpose::PrivateKeys).account(acct).object(acct).key(acct),
        )?;
        let identity = IdentityKeys::from_bytes(&raw)?;

        let mut vaults = HashMap::new();
        for row in self.store.vaults()? {
            let (h, key) = aead::unwrap_key(
                &ak,
                &row.wrapped_key,
                &Expect::purpose(Purpose::VaultKey).account(acct).container(acct).object(row.id).key(acct),
            )?;
            let key_gen = h.version as u32;
            let (_, attrs_raw) = aead::open(
                &key,
                &row.attrs,
                &Expect::purpose(Purpose::VaultAttrs).account(acct).container(row.id).object(row.id).key(row.id),
            )?;
            let attrs: VaultAttrs = serde_json::from_slice(&attrs_raw)?;
            vaults.insert(
                row.id,
                VaultState { key, key_gen, attrs, created_at: row.created_at, updated_at: row.updated_at, revision: row.revision },
            );
        }

        let mut items = HashMap::new();
        for row in self.store.items()? {
            let Some(vault) = vaults.get(&row.vault_id) else { continue };
            let (kind, overview) = open_overview(acct, &vault.key, &row)?;
            items.insert(
                row.id,
                CachedItem {
                    vault_id: row.vault_id,
                    kind,
                    overview,
                    revision: row.revision,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
            );
        }

        let local = match self.store.meta_get(LOCAL_STATE_META)? {
            Some(blob) => aead::open(&ak, &blob, &Expect::purpose(Purpose::LocalState).account(acct))
                .ok()
                .and_then(|(_, pt)| serde_json::from_slice(&pt).ok())
                .unwrap_or_default(),
            None => LocalState::default(),
        };

        self.session = Some(Session { record, ak, identity, vaults, items, local });
        self.store.log_event(event, None)?;
        self.purge_expired_trash()?;
        Ok(())
    }

    /// Forgets every key and decrypted value held in memory.
    pub fn lock(&mut self) {
        // Dropping the session zeroizes keys (ZeroizeOnDrop) and frees plaintext.
        self.session = None;
    }

    // ---- credentials management -------------------------------------------------------

    /// Verifies the master password without changing state (before sensitive actions).
    pub fn verify_password(&self, password: &str, secret_key: &SecretKey) -> Result<()> {
        let record = self.require_record()?;
        let muk = derive_unlock_key(password, secret_key, &record.kdf, &record.id)?;
        let expect = Expect::purpose(Purpose::AkByPassword).account(record.id).object(record.id);
        aead::unwrap_key(&muk, &record.wrapped_ak_password, &expect)
            .map(|_| ())
            .map_err(|_| CoreError::BadCredentials)
    }

    pub fn change_password(&mut self, current: &str, new: &str, secret_key: &SecretKey) -> Result<()> {
        self.session()?;
        self.verify_password(current, secret_key)?;
        check_master_password(new)?;
        self.set_password(new, secret_key)?;
        self.store.log_event("password.changed", None)
    }

    fn set_password(&mut self, new: &str, secret_key: &SecretKey) -> Result<()> {
        // Fresh salt and current recommended cost on every password change.
        let params = KdfParams::recommended();
        let s = self.session()?;
        let muk = derive_unlock_key(new, secret_key, &params, &s.record.id)?;
        let s = self.session_mut()?;
        s.record.kdf = params;
        s.record.wrapped_ak_password =
            aead::wrap_key(&muk, &ak_wrap_header(Purpose::AkByPassword, &s.record, s.record.id), &s.ak);
        s.record.password_changed_at = now_ms();
        self.save_record()
    }

    /// Resets the master password with the recovery code (and Secret Key). The code is
    /// single-use: it is rotated and the new one returned.
    pub fn recover(&mut self, code: &RecoveryCode, new_password: &str, secret_key: &SecretKey) -> Result<RecoveryCode> {
        check_master_password(new_password)?;
        let record = self.require_record()?;
        let wrapped = record
            .wrapped_ak_recovery
            .clone()
            .ok_or_else(|| CoreError::Invalid("o código de recuperação está desativado".into()))?;
        let expect = Expect::purpose(Purpose::AkByRecovery).account(record.id).object(record.id);
        let (_, ak) = aead::unwrap_key(&recovery_key(code, secret_key, &record.id), &wrapped, &expect)
            .map_err(|_| CoreError::BadCredentials)?;
        self.finish_unlock(record, ak, "unlock.recovery")?;
        self.set_password(new_password, secret_key)?;
        let fresh = self.rotate_recovery_code(secret_key)?;
        self.store.log_event("password.recovered", None)?;
        Ok(fresh)
    }

    pub fn rotate_recovery_code(&mut self, secret_key: &SecretKey) -> Result<RecoveryCode> {
        let code = RecoveryCode::generate();
        let s = self.session_mut()?;
        let rk = recovery_key(&code, secret_key, &s.record.id);
        s.record.wrapped_ak_recovery =
            Some(aead::wrap_key(&rk, &ak_wrap_header(Purpose::AkByRecovery, &s.record, s.record.id), &s.ak));
        s.record.recovery_created_at = Some(now_ms());
        self.save_record()?;
        self.store.log_event("recovery.rotated", None)?;
        Ok(code)
    }

    pub fn disable_recovery_code(&mut self) -> Result<()> {
        let s = self.session_mut()?;
        s.record.wrapped_ak_recovery = None;
        s.record.recovery_created_at = None;
        self.save_record()?;
        self.store.log_event("recovery.disabled", None)
    }

    // ---- device keys (Windows Hello and friends) --------------------------------------

    /// Stores a copy of the Account Key wrapped by a device-held key (e.g. derived from a
    /// Windows Hello signature). `label` identifies the device credential.
    pub fn enroll_device_key(&mut self, label: &str, device_key: &SymmetricKey) -> Result<()> {
        let s = self.session()?;
        let h = ak_wrap_header(Purpose::AkByDevice, &s.record, device_uuid(label));
        let wrapped = aead::wrap_key(device_key, &h, &s.ak);
        self.store.meta_set(&format!("{DEVICE_KEY_PREFIX}{label}"), &wrapped)?;
        self.store.log_event("device-key.enrolled", None)
    }

    pub fn unlock_with_device_key(&mut self, label: &str, device_key: &SymmetricKey) -> Result<()> {
        let record = self.require_record()?;
        let wrapped = self
            .store
            .meta_get(&format!("{DEVICE_KEY_PREFIX}{label}"))?
            .ok_or_else(|| CoreError::NotFound("chave do dispositivo".into()))?;
        let expect =
            Expect::purpose(Purpose::AkByDevice).account(record.id).container(device_uuid(label)).object(record.id);
        let (_, ak) = aead::unwrap_key(device_key, &wrapped, &expect).map_err(|_| CoreError::BadCredentials)?;
        self.finish_unlock(record, ak, "unlock.device")
    }

    pub fn remove_device_key(&mut self, label: &str) -> Result<()> {
        self.store.meta_delete(&format!("{DEVICE_KEY_PREFIX}{label}"))?;
        self.store.log_event("device-key.removed", None)
    }

    pub fn has_device_key(&self, label: &str) -> Result<bool> {
        Ok(self.store.meta_get(&format!("{DEVICE_KEY_PREFIX}{label}"))?.is_some())
    }

    pub fn device_key_labels(&self) -> Result<Vec<String>> {
        let mut labels = Vec::new();
        for candidate in ["windows-hello"] {
            if self.has_device_key(candidate)? {
                labels.push(candidate.to_string());
            }
        }
        Ok(labels)
    }

    // ---- vaults -----------------------------------------------------------------------

    pub fn vaults(&self) -> Result<Vec<VaultInfo>> {
        let s = self.session()?;
        let mut counts: HashMap<Uuid, usize> = HashMap::new();
        for it in s.items.values().filter(|i| i.overview.trashed_at.is_none()) {
            *counts.entry(it.vault_id).or_default() += 1;
        }
        let mut out: Vec<VaultInfo> = s
            .vaults
            .iter()
            .map(|(id, v)| VaultInfo {
                id: *id,
                attrs: v.attrs.clone(),
                created_at: v.created_at,
                updated_at: v.updated_at,
                item_count: counts.get(id).copied().unwrap_or(0),
            })
            .collect();
        out.sort_by_key(|v| v.created_at);
        Ok(out)
    }

    fn vault_row(&self, id: Uuid, v: &VaultState, attrs: &VaultAttrs, revision: u64, now: Timestamp) -> Result<VaultRow> {
        let s = self.session()?;
        let acct = s.record.id;
        let key_header = Header::new(Purpose::VaultKey, acct)
            .container(acct)
            .object(id)
            .key(acct, s.record.ak_gen)
            .version(v.key_gen as u64);
        let attrs_header =
            Header::new(Purpose::VaultAttrs, acct).container(id).object(id).key(id, v.key_gen).version(revision).padded();
        Ok(VaultRow {
            id,
            wrapped_key: aead::wrap_key(&s.ak, &key_header, &v.key),
            attrs: aead::seal(&v.key, &attrs_header, &serde_json::to_vec(attrs)?),
            created_at: v.created_at,
            updated_at: now,
            revision,
        })
    }

    pub fn create_vault(&mut self, attrs: VaultAttrs) -> Result<VaultInfo> {
        validate_vault_attrs(&attrs)?;
        let id = Uuid::new_v4();
        let now = now_ms();
        let state = VaultState { key: SymmetricKey::generate(), key_gen: 1, attrs: attrs.clone(), created_at: now, updated_at: now, revision: 1 };
        let row = self.vault_row(id, &state, &attrs, 1, now)?;
        self.store.put_vault(&row)?;
        self.session_mut()?.vaults.insert(id, state);
        Ok(VaultInfo { id, attrs, created_at: now, updated_at: now, item_count: 0 })
    }

    pub fn update_vault(&mut self, id: Uuid, attrs: VaultAttrs) -> Result<VaultInfo> {
        validate_vault_attrs(&attrs)?;
        let now = now_ms();
        let s = self.session()?;
        let v = s.vaults.get(&id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let row = self.vault_row(id, v, &attrs, v.revision + 1, now)?;
        self.store.put_vault(&row)?;
        let v = self.session_mut()?.vaults.get_mut(&id).expect("checked above");
        v.attrs = attrs;
        v.updated_at = now;
        v.revision += 1;
        self.vaults()?.into_iter().find(|v| v.id == id).ok_or_else(|| CoreError::NotFound("cofre".into()))
    }

    /// Deletes a vault. Its items move to `move_to` (re-encrypted there) or, if `None`,
    /// the vault must hold no live items (trashed ones are purged with it).
    pub fn delete_vault(&mut self, id: Uuid, move_to: Option<Uuid>) -> Result<()> {
        let s = self.session()?;
        if !s.vaults.contains_key(&id) {
            return Err(CoreError::NotFound("cofre".into()));
        }
        if s.vaults.len() == 1 {
            return Err(CoreError::Invalid("o Mocó precisa de pelo menos um cofre".into()));
        }
        let members: Vec<(Uuid, bool)> = s
            .items
            .iter()
            .filter(|(_, c)| c.vault_id == id)
            .map(|(i, c)| (*i, c.overview.trashed_at.is_some()))
            .collect();
        match move_to {
            Some(target) if target != id => {
                for (item, _) in &members {
                    self.move_item(*item, target)?;
                }
            }
            _ => {
                if members.iter().any(|(_, trashed)| !trashed) {
                    return Err(CoreError::Invalid("mova os itens deste cofre antes de apagá-lo".into()));
                }
                for (item, _) in &members {
                    self.purge_item(*item)?;
                }
            }
        }
        let rev = self.session()?.vaults[&id].revision + 1;
        self.store.delete_vault(id, rev, now_ms())?;
        self.session_mut()?.vaults.remove(&id);
        Ok(())
    }

    // ---- items: reads -----------------------------------------------------------------

    pub fn summaries(&self) -> Result<Vec<ItemSummary>> {
        let s = self.session()?;
        Ok(s.items
            .iter()
            .map(|(id, c)| {
                Item {
                    id: *id,
                    vault_id: c.vault_id,
                    kind: c.kind,
                    overview: c.overview.clone(),
                    details: Details::default(),
                    created_at: c.created_at,
                    updated_at: c.updated_at,
                    revision: c.revision,
                }
                .summary()
            })
            .collect())
    }

    pub fn item(&self, id: Uuid) -> Result<Item> {
        let s = self.session()?;
        let row = self.store.item(id)?.ok_or_else(|| CoreError::NotFound("item".into()))?;
        let vault = s.vaults.get(&row.vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        decrypt_item(s.record.id, &vault.key, &row)
    }

    /// Every live item with details — for health checks and export. Decrypts on demand.
    pub fn all_items(&self) -> Result<Vec<Item>> {
        let s = self.session()?;
        let mut out = Vec::new();
        for row in self.store.items()? {
            if let Some(vault) = s.vaults.get(&row.vault_id) {
                out.push(decrypt_item(s.record.id, &vault.key, &row)?);
            }
        }
        Ok(out)
    }

    pub fn history(&self, id: Uuid) -> Result<Vec<ItemVersion>> {
        let s = self.session()?;
        let mut out = Vec::new();
        for h in self.store.history(id)? {
            let Some(vault) = s.vaults.get(&h.vault_id) else { continue };
            let row = ItemRow {
                id: h.item_id,
                vault_id: h.vault_id,
                revision: h.revision,
                overview: h.overview.clone(),
                details: h.details.clone(),
                created_at: 0,
                updated_at: h.saved_at,
            };
            if let Ok(item) = decrypt_item(s.record.id, &vault.key, &row) {
                out.push(ItemVersion { revision: h.revision, saved_at: h.saved_at, item });
            }
        }
        Ok(out)
    }

    // ---- items: writes ----------------------------------------------------------------

    pub fn create_item(&mut self, vault_id: Uuid, input: ItemInput) -> Result<Item> {
        validate_input(&input)?;
        let now = now_ms();
        let mut item = Item {
            id: Uuid::new_v4(),
            vault_id,
            kind: input.kind,
            overview: Overview {
                title: input.title.trim().to_string(),
                urls: input.urls,
                tags: input.tags,
                favorite: input.favorite,
                icon: input.icon,
                ..Default::default()
            },
            details: Details { fields: clean_fields(input.fields), sections: input.sections, notes: input.notes, ..Default::default() },
            created_at: now,
            updated_at: now,
            revision: 1,
        };
        item.refresh_derived();
        self.write_item(&item, None)?;
        Ok(item)
    }

    /// Imports a fully-formed item (keeps timestamps, notes, password history, passkeys).
    pub fn import_item(&mut self, vault_id: Uuid, mut item: Item) -> Result<Item> {
        item.id = Uuid::new_v4();
        item.vault_id = vault_id;
        item.revision = 1;
        if item.created_at == 0 {
            item.created_at = now_ms();
        }
        if item.updated_at == 0 {
            item.updated_at = item.created_at;
        }
        if item.overview.title.trim().is_empty() {
            item.overview.title = "Sem nome".into();
        }
        item.details.fields = clean_fields(std::mem::take(&mut item.details.fields));
        item.refresh_derived();
        self.write_item(&item, None)?;
        Ok(item)
    }

    pub fn update_item(&mut self, id: Uuid, input: ItemInput) -> Result<Item> {
        validate_input(&input)?;
        let current = self.item(id)?;
        let mut next = current.clone();
        next.kind = input.kind;
        next.overview.title = input.title.trim().to_string();
        next.overview.urls = input.urls;
        next.overview.tags = input.tags;
        next.overview.favorite = input.favorite;
        next.overview.icon = input.icon;
        next.details.fields = clean_fields(input.fields);
        next.details.sections = input.sections;
        next.details.notes = input.notes;

        // Never silently lose an old password.
        let now = now_ms();
        for f in current.details.fields.iter().filter(|f| f.kind == FieldType::Concealed && !f.value.is_empty()) {
            let still_same = next.details.field(&f.id).is_some_and(|n| n.value == f.value);
            if !still_same {
                next.details.password_history.insert(0, PasswordHistoryEntry { value: f.value.clone(), replaced_at: now });
            }
        }
        next.details.password_history.truncate(PASSWORD_HISTORY_KEEP);
        next.refresh_derived();
        if next.overview == current.overview && next.details == current.details && next.kind == current.kind {
            return Ok(current);
        }
        next.updated_at = now;
        next.revision = current.revision + 1;
        self.write_item(&next, Some(&current))?;
        Ok(next)
    }

    fn mutate_overview(&mut self, id: Uuid, f: impl FnOnce(&mut Overview)) -> Result<Item> {
        let current = self.item(id)?;
        let mut next = current.clone();
        f(&mut next.overview);
        if next.overview == current.overview {
            return Ok(current);
        }
        next.revision = current.revision + 1;
        next.updated_at = now_ms();
        // Flag-only changes don't create a history version.
        self.write_item(&next, None)?;
        Ok(next)
    }

    pub fn set_favorite(&mut self, id: Uuid, favorite: bool) -> Result<Item> {
        self.mutate_overview(id, |o| o.favorite = favorite)
    }

    pub fn set_archived(&mut self, id: Uuid, archived: bool) -> Result<Item> {
        self.mutate_overview(id, |o| o.archived = archived)
    }

    pub fn trash_item(&mut self, id: Uuid) -> Result<Item> {
        let now = now_ms();
        self.mutate_overview(id, |o| o.trashed_at = Some(now))
    }

    pub fn restore_item(&mut self, id: Uuid) -> Result<Item> {
        self.mutate_overview(id, |o| o.trashed_at = None)
    }

    /// Permanently deletes an item, leaving an authenticated tombstone for sync.
    pub fn purge_item(&mut self, id: Uuid) -> Result<()> {
        let s = self.session()?;
        let cached = s.items.get(&id).ok_or_else(|| CoreError::NotFound("item".into()))?;
        let vault = s.vaults.get(&cached.vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let revision = cached.revision + 1;
        let tombstone = aead::seal(
            &vault.key,
            &Header::new(Purpose::Tombstone, s.record.id)
                .container(cached.vault_id)
                .object(id)
                .key(cached.vault_id, vault.key_gen)
                .version(revision),
            &[],
        );
        self.store.purge_item(id, revision, now_ms(), &tombstone)?;
        self.store.delete_attachments_of(id)?;
        let s = self.session_mut()?;
        s.items.remove(&id);
        if s.local.usage.remove(&id).is_some() {
            self.save_local()?;
        }
        Ok(())
    }

    pub fn empty_trash(&mut self) -> Result<usize> {
        let ids: Vec<Uuid> =
            self.session()?.items.iter().filter(|(_, c)| c.overview.trashed_at.is_some()).map(|(id, _)| *id).collect();
        for id in &ids {
            self.purge_item(*id)?;
        }
        Ok(ids.len())
    }

    fn purge_expired_trash(&mut self) -> Result<()> {
        let cutoff = now_ms() - TRASH_RETENTION_DAYS * 24 * 3600 * 1000;
        let ids: Vec<Uuid> = self
            .session()?
            .items
            .iter()
            .filter(|(_, c)| c.overview.trashed_at.is_some_and(|t| t < cutoff))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.purge_item(id)?;
        }
        Ok(())
    }

    pub fn move_item(&mut self, id: Uuid, vault_id: Uuid) -> Result<Item> {
        let current = self.item(id)?;
        if current.vault_id == vault_id {
            return Ok(current);
        }
        let s = self.session()?;
        let acct = s.record.id;
        let target = s.vaults.get(&vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let source = s.vaults.get(&current.vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;

        // History is bound to the old vault key: re-encrypt it under the new one.
        let mut history = Vec::new();
        for h in self.store.history(id)? {
            let row = ItemRow {
                id: h.item_id,
                vault_id: h.vault_id,
                revision: h.revision,
                overview: h.overview,
                details: h.details,
                created_at: 0,
                updated_at: h.saved_at,
            };
            if let Ok(mut old) = decrypt_item(acct, &source.key, &row) {
                old.vault_id = vault_id;
                let enc = encrypt_item(acct, &target.key, target.key_gen, &old)?;
                history.push(HistoryRow {
                    item_id: id,
                    vault_id,
                    revision: h.revision,
                    overview: enc.overview,
                    details: enc.details,
                    saved_at: h.saved_at,
                });
            }
        }

        let mut next = current.clone();
        next.vault_id = vault_id;
        next.revision = current.revision + 1;
        next.updated_at = now_ms();
        let row = encrypt_item(acct, &target.key, target.key_gen, &next)?;
        self.move_attachments(id, current.vault_id, vault_id)?;
        self.store.transaction(|st| {
            st.put_item(&row)?;
            st.replace_history(id, &history)
        })?;
        self.cache(&next);
        Ok(next)
    }

    pub fn duplicate_item(&mut self, id: Uuid) -> Result<Item> {
        let current = self.item(id)?;
        let now = now_ms();
        let mut copy = current.clone();
        copy.id = Uuid::new_v4();
        copy.overview.title = format!("{} (cópia)", current.overview.title);
        copy.overview.favorite = false;
        copy.overview.trashed_at = None;
        copy.details.password_history.clear();
        copy.details.attachments.clear();
        copy.created_at = now;
        copy.updated_at = now;
        copy.revision = 1;
        copy.refresh_derived();
        self.write_item(&copy, None)?;
        Ok(copy)
    }

    pub fn restore_version(&mut self, id: Uuid, revision: u64) -> Result<Item> {
        let version = self
            .history(id)?
            .into_iter()
            .find(|v| v.revision == revision)
            .ok_or_else(|| CoreError::NotFound("versão".into()))?;
        let old = version.item;
        let input = ItemInput {
            kind: old.kind,
            title: old.overview.title,
            urls: old.overview.urls,
            tags: old.overview.tags,
            favorite: old.overview.favorite,
            icon: old.overview.icon,
            fields: old.details.fields,
            sections: old.details.sections,
            notes: old.details.notes,
        };
        self.update_item(id, input)
    }

    fn write_item(&mut self, item: &Item, previous: Option<&Item>) -> Result<()> {
        let s = self.session()?;
        let acct = s.record.id;
        let vault = s.vaults.get(&item.vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let row = encrypt_item(acct, &vault.key, vault.key_gen, item)?;
        let history = match previous {
            Some(prev) => {
                let pv = s.vaults.get(&prev.vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
                let enc = encrypt_item(acct, &pv.key, pv.key_gen, prev)?;
                Some(HistoryRow {
                    item_id: prev.id,
                    vault_id: prev.vault_id,
                    revision: prev.revision,
                    overview: enc.overview,
                    details: enc.details,
                    saved_at: prev.updated_at,
                })
            }
            None => None,
        };
        self.store.transaction(|st| {
            st.put_item(&row)?;
            if let Some(h) = &history {
                st.push_history(h, HISTORY_KEEP)?;
            }
            Ok(())
        })?;
        self.cache(item);
        Ok(())
    }

    fn cache(&mut self, item: &Item) {
        if let Some(s) = self.session.as_mut() {
            s.items.insert(
                item.id,
                CachedItem {
                    vault_id: item.vault_id,
                    kind: item.kind,
                    overview: item.overview.clone(),
                    revision: item.revision,
                    created_at: item.created_at,
                    updated_at: item.updated_at,
                },
            );
        }
    }

    // ---- attachments ------------------------------------------------------------------

    fn attachment_header(&self, vault: Uuid, key_gen: u32, item: Uuid, att: Uuid) -> Result<Header> {
        let acct = self.session()?.record.id;
        Ok(Header::new(Purpose::Attachment, acct).container(item).object(att).key(vault, key_gen).padded())
    }

    /// Encrypts and attaches a file to an item (max 25 MB).
    pub fn add_attachment(&mut self, item_id: Uuid, name: &str, mime: &str, bytes: &[u8]) -> Result<Item> {
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(CoreError::Invalid("o arquivo passa de 25 MB".into()));
        }
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::Invalid("arquivo sem nome".into()));
        }
        let current = self.item(item_id)?;
        let s = self.session()?;
        let vault = s.vaults.get(&current.vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let att = Uuid::new_v4();
        let blob = aead::seal(&vault.key, &self.attachment_header(current.vault_id, vault.key_gen, item_id, att)?, bytes);
        let now = now_ms();
        self.store.put_attachment(att, item_id, current.vault_id, &blob, now)?;
        let mut next = current.clone();
        next.details.attachments.push(AttachmentMeta { id: att, name: name.to_string(), size: bytes.len() as u64, mime: mime.to_string(), added_at: now });
        next.revision = current.revision + 1;
        next.updated_at = now;
        next.refresh_derived();
        self.write_item(&next, Some(&current))?;
        Ok(next)
    }

    pub fn read_attachment(&self, item_id: Uuid, att_id: Uuid) -> Result<(AttachmentMeta, Zeroizing<Vec<u8>>)> {
        let item = self.item(item_id)?;
        let meta = item.details.attachments.iter().find(|a| a.id == att_id).cloned().ok_or_else(|| CoreError::NotFound("anexo".into()))?;
        let (owner, vault_id, blob) = self.store.attachment(att_id)?.ok_or_else(|| CoreError::NotFound("anexo".into()))?;
        if owner != item_id || vault_id != item.vault_id {
            return Err(CoreError::Integrity);
        }
        let s = self.session()?;
        let vault = s.vaults.get(&vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let acct = s.record.id;
        let (_, pt) = aead::open(&vault.key, &blob, &Expect::purpose(Purpose::Attachment).account(acct).container(item_id).object(att_id).key(vault_id))?;
        Ok((meta, pt))
    }

    pub fn remove_attachment(&mut self, item_id: Uuid, att_id: Uuid) -> Result<Item> {
        let current = self.item(item_id)?;
        let mut next = current.clone();
        next.details.attachments.retain(|a| a.id != att_id);
        if next.details.attachments.len() == current.details.attachments.len() {
            return Err(CoreError::NotFound("anexo".into()));
        }
        next.revision = current.revision + 1;
        next.updated_at = now_ms();
        next.refresh_derived();
        self.write_item(&next, Some(&current))?;
        self.store.delete_attachment(att_id)?;
        Ok(next)
    }

    /// Re-encrypts an item's attachments for another vault (called by move).
    fn move_attachments(&self, item_id: Uuid, from: Uuid, to: Uuid) -> Result<()> {
        let s = self.session()?;
        let src = s.vaults.get(&from).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let dst = s.vaults.get(&to).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let acct = s.record.id;
        for (att, _, blob) in self.store.attachments_of(item_id)? {
            let (_, pt) = aead::open(&src.key, &blob, &Expect::purpose(Purpose::Attachment).account(acct).container(item_id).object(att).key(from))?;
            let sealed = aead::seal(&dst.key, &self.attachment_header(to, dst.key_gen, item_id, att)?, &pt);
            self.store.put_attachment(att, item_id, to, &sealed, now_ms())?;
        }
        Ok(())
    }

    // ---- local usage (recents) --------------------------------------------------------

    pub fn touch(&mut self, id: Uuid) -> Result<()> {
        let s = self.session_mut()?;
        if !s.items.contains_key(&id) {
            return Err(CoreError::NotFound("item".into()));
        }
        let u = s.local.usage.entry(id).or_default();
        u.last_used_at = now_ms();
        u.count = u.count.saturating_add(1);
        self.save_local()
    }

    pub fn usage(&self) -> Result<HashMap<Uuid, Usage>> {
        Ok(self.session()?.local.usage.clone())
    }

    fn save_local(&self) -> Result<()> {
        let s = self.session()?;
        let acct = s.record.id;
        let h = Header::new(Purpose::LocalState, acct).container(acct).object(acct).key(acct, s.record.ak_gen).padded();
        let blob = aead::seal(&s.ak, &h, &serde_json::to_vec(&s.local)?);
        self.store.meta_set(LOCAL_STATE_META, &blob)
    }

    pub fn public_identity(&self) -> Result<PublicIdentity> {
        Ok(self.session()?.record.public_identity.clone())
    }

    pub fn account_id(&self) -> Result<Uuid> {
        Ok(self.session()?.record.id)
    }

    /// Signs a message with the account's Ed25519 identity key.
    pub fn sign(&self, message: &[u8]) -> Result<[u8; 64]> {
        Ok(self.session()?.identity.sign(message))
    }

    /// Wraps arbitrary bytes under the Account Key (used for device-local secrets that
    /// should only be readable while unlocked).
    pub fn seal_with_account_key(&self, purpose: Purpose, object: Uuid, plaintext: &[u8]) -> Result<Vec<u8>> {
        let s = self.session()?;
        let acct = s.record.id;
        let h = Header::new(purpose, acct).container(acct).object(object).key(acct, s.record.ak_gen).padded();
        Ok(aead::seal(&s.ak, &h, plaintext))
    }

    pub fn open_with_account_key(&self, purpose: Purpose, object: Uuid, envelope: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let s = self.session()?;
        let acct = s.record.id;
        let (_, pt) = aead::open(&s.ak, envelope, &Expect::purpose(purpose).account(acct).object(object))?;
        Ok(pt)
    }
}

// ---- item encryption ----------------------------------------------------------------

struct EncryptedItem {
    overview: Vec<u8>,
    details: Vec<u8>,
}

fn encrypt_item(account: Uuid, key: &SymmetricKey, key_gen: u32, item: &Item) -> Result<ItemRow> {
    let write_id = random::bytes::<16>();
    let overview = Zeroizing::new(serde_json::to_vec(&SealedOverview { kind: item.kind, overview: item.overview.clone() })?);
    let details = Zeroizing::new(serde_json::to_vec(&item.details)?);
    let enc = EncryptedItem {
        overview: aead::seal(key, &item_header(Purpose::ItemOverview, account, item, key_gen, write_id), &overview),
        details: aead::seal(key, &item_header(Purpose::ItemDetails, account, item, key_gen, write_id), &details),
    };
    Ok(ItemRow {
        id: item.id,
        vault_id: item.vault_id,
        revision: item.revision,
        overview: enc.overview,
        details: enc.details,
        created_at: item.created_at,
        updated_at: item.updated_at,
    })
}

fn open_overview(account: Uuid, key: &SymmetricKey, row: &ItemRow) -> Result<(ItemKind, Overview)> {
    let (h, pt) = aead::open(key, &row.overview, &item_expect(Purpose::ItemOverview, account, row.vault_id, row.id))?;
    // The revision column is only a cache of the authenticated version.
    if h.version != row.revision {
        return Err(CoreError::Integrity);
    }
    let sealed: SealedOverview = serde_json::from_slice(&pt)?;
    Ok((sealed.kind, sealed.overview))
}

fn decrypt_item(account: Uuid, key: &SymmetricKey, row: &ItemRow) -> Result<Item> {
    let (oh, opt) = aead::open(key, &row.overview, &item_expect(Purpose::ItemOverview, account, row.vault_id, row.id))?;
    let (dh, dpt) = aead::open(key, &row.details, &item_expect(Purpose::ItemDetails, account, row.vault_id, row.id))?;
    // Overview and details must come from the same logical write.
    if oh.write_id != dh.write_id || oh.version != dh.version || oh.version != row.revision {
        return Err(CoreError::Integrity);
    }
    let sealed: SealedOverview = serde_json::from_slice(&opt)?;
    let details: Details = serde_json::from_slice(&dpt)?;
    Ok(Item {
        id: row.id,
        vault_id: row.vault_id,
        kind: sealed.kind,
        overview: sealed.overview,
        details,
        created_at: row.created_at,
        updated_at: row.updated_at,
        revision: row.revision,
    })
}

// ---- validation ---------------------------------------------------------------------

pub fn check_master_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(CoreError::Invalid(format!("a senha mestra precisa de pelo menos {MIN_PASSWORD_CHARS} caracteres")));
    }
    Ok(())
}

fn validate_vault_attrs(a: &VaultAttrs) -> Result<()> {
    let n = a.name.trim();
    if n.is_empty() {
        return Err(CoreError::Invalid("dê um nome ao cofre".into()));
    }
    if n.chars().count() > 60 {
        return Err(CoreError::Invalid("nome do cofre muito longo".into()));
    }
    Ok(())
}

fn validate_input(i: &ItemInput) -> Result<()> {
    if i.title.trim().is_empty() {
        return Err(CoreError::Invalid("dê um nome ao item".into()));
    }
    if i.title.chars().count() > 200 {
        return Err(CoreError::Invalid("nome muito longo".into()));
    }
    Ok(())
}

fn clean_fields(fields: Vec<Field>) -> Vec<Field> {
    fields
        .into_iter()
        .map(|mut f| {
            if f.id.trim().is_empty() {
                f.id = Uuid::new_v4().simple().to_string()[..12].to_string();
            }
            f.label = f.label.trim().to_string();
            f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretString;

    fn personal() -> VaultAttrs {
        VaultAttrs { name: "Pessoal".into(), description: String::new(), icon: "house".into(), color: "clay".into() }
    }

    fn setup() -> (Account, SecretKey, RecoveryCode) {
        let mut acct = Account::new(Store::open_in_memory().unwrap());
        acct.upgrade_kdf_on_unlock = false;
        let sk = SecretKey::generate();
        let rc = acct.create_with_params("correct horse battery", &sk, personal(), KdfParams::fast_for_tests()).unwrap();
        (acct, sk, rc)
    }

    fn login(title: &str, user: &str, pass: &str) -> ItemInput {
        ItemInput {
            kind: ItemKind::Login,
            title: title.into(),
            urls: vec!["https://github.com".into()],
            tags: vec!["Trabalho".into()],
            favorite: false,
            icon: None,
            fields: vec![
                Field { id: "username".into(), label: String::new(), kind: FieldType::Text, value: SecretString::new(user), section: None },
                Field { id: "password".into(), label: String::new(), kind: FieldType::Concealed, value: SecretString::new(pass), section: None },
            ],
            sections: vec![],
            notes: SecretString::default(),
        }
    }

    #[test]
    fn create_lock_unlock() {
        let (mut acct, sk, _) = setup();
        let vault = acct.vaults().unwrap()[0].id;
        let item = acct.create_item(vault, login("GitHub", "ana", "s3nh@-f0rte")).unwrap();
        acct.lock();
        assert!(matches!(acct.summaries(), Err(CoreError::Locked)));

        assert!(matches!(acct.unlock("wrong password", &sk), Err(CoreError::BadCredentials)));
        assert!(matches!(acct.unlock("correct horse battery", &SecretKey::generate()), Err(CoreError::BadCredentials)));

        acct.unlock("correct horse battery", &sk).unwrap();
        let got = acct.item(item.id).unwrap();
        assert_eq!(got.details.value("password"), Some("s3nh@-f0rte"));
        assert_eq!(got.overview.subtitle, "ana");
        assert_eq!(acct.summaries().unwrap().len(), 1);
        assert_eq!(acct.vaults().unwrap()[0].item_count, 1);
    }

    #[test]
    fn weak_kdf_is_upgraded_on_unlock() {
        let (mut acct, sk, _) = setup();
        acct.upgrade_kdf_on_unlock = true;
        acct.lock();
        acct.unlock("correct horse battery", &sk).unwrap();
        let rec = acct.record().unwrap().unwrap();
        assert_eq!(rec.kdf.memory_kib, KdfParams::recommended().memory_kib);
        acct.lock();
        acct.unlock("correct horse battery", &sk).unwrap();
    }

    #[test]
    fn nothing_in_plaintext_on_disk() {
        let dir = std::env::temp_dir().join(format!("moco-test-{}", Uuid::new_v4()));
        let path = dir.join("vault.db");
        {
            let mut acct = Account::new(Store::open(&path).unwrap());
            let sk = SecretKey::generate();
            acct.create_with_params("correct horse battery", &sk, personal(), KdfParams::fast_for_tests()).unwrap();
            let vault = acct.vaults().unwrap()[0].id;
            acct.create_item(vault, login("NubankSecretTitle", "usuario.secreto", "SenhaUltraSecreta123")).unwrap();
        }
        let mut raw = std::fs::read(&path).unwrap();
        if let Ok(wal) = std::fs::read(dir.join("vault.db-wal")) {
            raw.extend(wal);
        }
        let hay = String::from_utf8_lossy(&raw);
        for needle in ["NubankSecretTitle", "usuario.secreto", "SenhaUltraSecreta123", "Pessoal", "github.com"] {
            assert!(!hay.contains(needle), "{needle} found in plaintext on disk");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn password_change_and_recovery() {
        let (mut acct, sk, rc) = setup();
        assert!(acct.change_password("correct horse battery", "curta", &sk).is_err());
        acct.change_password("correct horse battery", "nova senha mestra", &sk).unwrap();
        acct.lock();
        assert!(acct.unlock("correct horse battery", &sk).is_err());
        acct.unlock("nova senha mestra", &sk).unwrap();
        acct.lock();

        // Recovery needs the Secret Key too.
        assert!(acct.recover(&rc, "outra senha forte", &SecretKey::generate()).is_err());
        let fresh = acct.recover(&rc, "outra senha forte", &sk).unwrap();
        assert!(acct.is_unlocked());
        acct.lock();
        // Old recovery code no longer works after rotation (single use).
        assert!(acct.recover(&rc, "tentativa longa", &sk).is_err());
        acct.unlock("outra senha forte", &sk).unwrap();
        acct.lock();
        acct.recover(&fresh, "mais uma senha", &sk).unwrap();
        acct.disable_recovery_code().unwrap();
        assert!(!acct.status().unwrap().recovery_enabled);
    }

    #[test]
    fn device_key_unlock() {
        let (mut acct, _, _) = setup();
        let dk = SymmetricKey::generate();
        acct.enroll_device_key("windows-hello", &dk).unwrap();
        acct.lock();
        assert!(acct.unlock_with_device_key("windows-hello", &SymmetricKey::generate()).is_err());
        acct.unlock_with_device_key("windows-hello", &dk).unwrap();
        assert_eq!(acct.status().unwrap().device_keys, vec!["windows-hello".to_string()]);
    }

    #[test]
    fn edits_keep_history_and_password_history() {
        let (mut acct, _, _) = setup();
        let vault = acct.vaults().unwrap()[0].id;
        let item = acct.create_item(vault, login("GitHub", "ana", "primeira")).unwrap();
        let updated = acct.update_item(item.id, login("GitHub", "ana", "segunda")).unwrap();
        assert_eq!(updated.revision, 2);
        assert_eq!(updated.details.password_history.len(), 1);
        assert_eq!(updated.details.password_history[0].value.expose(), "primeira");
        let hist = acct.history(item.id).unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].item.details.value("password"), Some("primeira"));
        let restored = acct.restore_version(item.id, 1).unwrap();
        assert_eq!(restored.details.value("password"), Some("primeira"));

        // No-op update doesn't bump the revision.
        let same = acct.update_item(item.id, login("GitHub", "ana", "primeira")).unwrap();
        assert_eq!(same.revision, restored.revision);
    }

    #[test]
    fn move_trash_restore_purge() {
        let (mut acct, _, _) = setup();
        let personal_id = acct.vaults().unwrap()[0].id;
        let work = acct
            .create_vault(VaultAttrs { name: "Trabalho".into(), description: String::new(), icon: "briefcase".into(), color: "ink".into() })
            .unwrap();
        let item = acct.create_item(personal_id, login("Slack", "ana", "a")).unwrap();
        acct.update_item(item.id, login("Slack", "ana", "b")).unwrap();
        let moved = acct.move_item(item.id, work.id).unwrap();
        assert_eq!(moved.vault_id, work.id);
        assert_eq!(acct.history(item.id).unwrap().len(), 1, "history survives a move");

        acct.trash_item(item.id).unwrap();
        assert!(acct.summaries().unwrap()[0].trashed_at.is_some());
        acct.restore_item(item.id).unwrap();
        assert!(acct.summaries().unwrap()[0].trashed_at.is_none());
        acct.trash_item(item.id).unwrap();
        assert_eq!(acct.empty_trash().unwrap(), 1);
        assert!(acct.summaries().unwrap().is_empty());
        assert!(acct.item(item.id).is_err());

        assert!(acct.delete_vault(work.id, None).is_ok());
        assert!(acct.delete_vault(personal_id, None).is_err(), "last vault can't be deleted");
    }

    #[test]
    fn tampered_rows_are_rejected() {
        let (mut acct, _, _) = setup();
        let vault = acct.vaults().unwrap()[0].id;
        let a = acct.create_item(vault, login("A", "a", "a")).unwrap();
        let b = acct.create_item(vault, login("B", "b", "b")).unwrap();

        // Swap B's ciphertext into A's row: header binding rejects it.
        let row_b = acct.store.item(b.id).unwrap().unwrap();
        let mut forged = acct.store.item(a.id).unwrap().unwrap();
        let original = forged.clone();
        forged.overview = row_b.overview.clone();
        forged.details = row_b.details.clone();
        acct.store.put_item(&forged).unwrap();
        assert!(acct.item(a.id).is_err());

        // Mixing overview/details from two different writes of the same item is rejected.
        acct.store.put_item(&original).unwrap();
        acct.update_item(a.id, login("A", "a", "nova")).unwrap();
        let newer = acct.store.item(a.id).unwrap().unwrap();
        let mut mixed = newer.clone();
        mixed.details = original.details.clone();
        acct.store.put_item(&mixed).unwrap();
        assert!(acct.item(a.id).is_err());

        // Relabelling an old version with the current revision number is rejected.
        let mut rolled = original.clone();
        rolled.revision = newer.revision;
        acct.store.put_item(&rolled).unwrap();
        assert!(acct.item(a.id).is_err());
    }

    #[test]
    fn attachments_roundtrip_move_and_purge() {
        let (mut acct, _, _) = setup();
        let personal = acct.vaults().unwrap()[0].id;
        let work = acct.create_vault(VaultAttrs { name: "T".into(), description: String::new(), icon: String::new(), color: String::new() }).unwrap().id;
        let item = acct.create_item(personal, login("RG", "a", "b")).unwrap();
        let with = acct.add_attachment(item.id, "rg-frente.jpg", "image/jpeg", b"imagem secreta").unwrap();
        assert_eq!(with.overview.attachment_count, 1);
        let att = with.details.attachments[0].id;
        assert_eq!(&acct.read_attachment(item.id, att).unwrap().1[..], b"imagem secreta");
        acct.move_item(item.id, work).unwrap();
        assert_eq!(&acct.read_attachment(item.id, att).unwrap().1[..], b"imagem secreta");
        acct.remove_attachment(item.id, att).unwrap();
        assert!(acct.read_attachment(item.id, att).is_err());
        let again = acct.add_attachment(item.id, "x.pdf", "application/pdf", b"pdf").unwrap();
        let att2 = again.details.attachments[0].id;
        acct.purge_item(item.id).unwrap();
        assert!(acct.store.attachment(att2).unwrap().is_none());
    }

    #[test]
    fn usage_is_tracked_and_persisted() {
        let (mut acct, sk, _) = setup();
        let vault = acct.vaults().unwrap()[0].id;
        let item = acct.create_item(vault, login("A", "a", "a")).unwrap();
        acct.touch(item.id).unwrap();
        acct.touch(item.id).unwrap();
        acct.lock();
        acct.unlock("correct horse battery", &sk).unwrap();
        assert_eq!(acct.usage().unwrap()[&item.id].count, 2);
    }
}
