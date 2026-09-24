//! Synchronization engine — transport-agnostic.
//!
//! The server stores opaque ciphertext and does optimistic concurrency: a push carries the
//! version it was based on and is accepted only if that is still current; otherwise the
//! server answers with its current row. Everything the server sends is authenticated
//! before use (envelope headers bind account, container, object and version), and the
//! client refuses any version lower than the highest it has already seen for that object
//! (per-object high-water mark), so a malicious or broken server cannot roll items back,
//! swap them between vaults, or forge deletions.
//!
//! Conflicts (same item changed on two devices before syncing) resolve without asking:
//! the most recently edited version wins, and the other one is kept in the item's
//! history — nothing is silently lost.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    decrypt_item, encrypt_item, Account, AccountRecord, CachedItem, VaultAttrs, VaultState, ACCOUNT_META,
    HISTORY_KEEP,
};
use crate::crypto::aead::{self, Expect, Purpose};
use crate::error::{CoreError, Result};
use crate::model::{now_ms, Item, Timestamp};
use crate::store::{HistoryRow, ItemRow, VaultRow};

const E_ITEM: &str = "item";
const E_VAULT: &str = "vault";
const CURSOR_META: &str = "sync-cursor";
const RECORD_VERSION_META: &str = "account-record-version";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WireVault {
    pub id: Uuid,
    pub version: u64,
    #[serde(with = "crate::util::b64")]
    pub wrapped_key: Vec<u8>,
    /// Encrypted attributes, or the authenticated tombstone when deleted.
    #[serde(with = "crate::util::b64")]
    pub attrs: Vec<u8>,
    pub deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WireItem {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub version: u64,
    /// Encrypted overview, or the authenticated tombstone when deleted.
    #[serde(with = "crate::util::b64")]
    pub overview: Vec<u8>,
    #[serde(with = "crate::util::b64")]
    pub details: Vec<u8>,
    pub deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Push<T> {
    /// Server version this change was based on (0 = new).
    pub base: u64,
    pub row: T,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PushRequest {
    pub vaults: Vec<Push<WireVault>>,
    pub items: Vec<Push<WireItem>>,
}

impl PushRequest {
    pub fn is_empty(&self) -> bool {
        self.vaults.is_empty() && self.items.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PushResponse {
    /// (entity, id, version) accepted and stored by the server.
    pub accepted: Vec<(String, Uuid, u64)>,
    /// Current server rows for pushes whose base was stale.
    pub conflict_vaults: Vec<WireVault>,
    pub conflict_items: Vec<WireItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PullResponse {
    pub cursor: i64,
    pub vaults: Vec<WireVault>,
    pub items: Vec<WireItem>,
    /// The encrypted account record, when it changed (password change elsewhere).
    #[serde(default)]
    pub record: Option<String>,
    #[serde(default)]
    pub record_version: u64,
}

/// What the UI shows after a sync.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub pushed: usize,
    pub pulled: usize,
    pub conflicts_resolved: usize,
    /// Rows the server sent that failed authentication or tried to go back in time.
    pub rejected: usize,
}

pub(super) fn wire_item(row: &ItemRow, deleted: bool) -> WireItem {
    to_wire_item(row, deleted)
}

fn to_wire_item(row: &ItemRow, deleted: bool) -> WireItem {
    WireItem {
        id: row.id,
        vault_id: row.vault_id,
        version: row.revision,
        overview: row.overview.clone(),
        details: if deleted { Vec::new() } else { row.details.clone() },
        deleted,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn from_wire_item(w: &WireItem) -> ItemRow {
    ItemRow {
        id: w.id,
        vault_id: w.vault_id,
        revision: w.version,
        overview: w.overview.clone(),
        details: w.details.clone(),
        created_at: w.created_at,
        updated_at: w.updated_at,
    }
}

fn to_wire_vault(row: &VaultRow, deleted: bool) -> WireVault {
    WireVault {
        id: row.id,
        version: row.revision,
        wrapped_key: row.wrapped_key.clone(),
        attrs: row.attrs.clone(),
        deleted,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn from_wire_vault(w: &WireVault) -> VaultRow {
    VaultRow {
        id: w.id,
        wrapped_key: w.wrapped_key.clone(),
        attrs: w.attrs.clone(),
        created_at: w.created_at,
        updated_at: w.updated_at,
        revision: w.version,
    }
}

enum Verified {
    Vault(Option<(crate::crypto::SymmetricKey, u32, VaultAttrs)>),
    Item(Option<Item>),
}

impl Account {
    // ---- account record ---------------------------------------------------------------

    /// The encrypted account record, as uploaded to the server at sign-up.
    pub fn record_json(&self) -> Result<String> {
        let raw = self.store.meta_get(ACCOUNT_META)?.ok_or_else(|| CoreError::NotFound("conta".into()))?;
        Ok(String::from_utf8(raw).map_err(|_| CoreError::Integrity)?)
    }

    pub fn record_version(&self) -> Result<u64> {
        Ok(self
            .store
            .meta_get(RECORD_VERSION_META)?
            .and_then(|v| String::from_utf8(v).ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(1))
    }

    pub fn set_record_version(&self, v: u64) -> Result<()> {
        self.store.meta_set(RECORD_VERSION_META, v.to_string().as_bytes())
    }

    /// Installs an account record downloaded from the server on a new device.
    pub fn install_record(&mut self, record_json: &str, expected_id: Uuid, version: u64) -> Result<()> {
        if self.is_initialized()? {
            return Err(CoreError::Conflict("já existe um Mocó neste computador".into()));
        }
        let rec: AccountRecord = serde_json::from_str(record_json)?;
        if rec.id != expected_id {
            return Err(CoreError::Integrity);
        }
        rec.kdf.validate()?;
        self.store.meta_set(ACCOUNT_META, record_json.as_bytes())?;
        self.set_record_version(version)
    }

    /// Accepts a newer record from the server (e.g. password changed on another device)
    /// only if it is provably ours: its identity keys decrypt under our Account Key.
    fn apply_record(&mut self, record_json: &str, version: u64) -> Result<bool> {
        if version <= self.record_version()? {
            return Ok(false);
        }
        let rec: AccountRecord = serde_json::from_str(record_json)?;
        let s = self.session()?;
        if rec.id != s.record.id {
            return Err(CoreError::Integrity);
        }
        rec.kdf.validate()?;
        aead::open(&s.ak, &rec.encrypted_identity, &Expect::purpose(Purpose::PrivateKeys).account(rec.id).object(rec.id))?;
        self.store.meta_set(ACCOUNT_META, record_json.as_bytes())?;
        self.set_record_version(version)?;
        self.session_mut()?.record = rec;
        let _ = self.store.log_event("sync.record", None);
        Ok(true)
    }

    // ---- cursor -----------------------------------------------------------------------

    pub fn sync_cursor(&self) -> Result<i64> {
        Ok(self.store.meta_get(CURSOR_META)?.and_then(|v| String::from_utf8(v).ok()).and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    fn set_cursor(&self, c: i64) -> Result<()> {
        self.store.meta_set(CURSOR_META, c.to_string().as_bytes())
    }

    /// Forget sync progress (e.g. after signing out) so the next sign-in re-pulls.
    pub fn reset_sync_state(&self) -> Result<()> {
        self.set_cursor(0)
    }

    // ---- push -------------------------------------------------------------------------

    /// Local changes not yet on the server. `upto` must be passed back to
    /// [`Account::sync_apply_push`] so changes made meanwhile aren't marked as sent.
    pub fn sync_collect(&self) -> Result<(i64, PushRequest)> {
        self.session()?;
        let (upto, pending) = self.store.outbox_pending()?;
        let mut req = PushRequest::default();
        for (entity, id) in pending {
            match entity.as_str() {
                E_VAULT => {
                    if let Some((row, deleted)) = self.store.vault_raw(id)? {
                        let base = self.store.hwm(E_VAULT, id)?;
                        if row.revision > base {
                            req.vaults.push(Push { base, row: to_wire_vault(&row, deleted) });
                        }
                    }
                }
                _ => {
                    if let Some((row, deleted)) = self.store.item_raw(id)? {
                        if self.is_shared_with_us(row.vault_id) {
                            continue;
                        }
                        let base = self.store.hwm(E_ITEM, id)?;
                        if row.revision > base {
                            req.items.push(Push { base, row: to_wire_item(&row, deleted) });
                        }
                    }
                }
            }
        }
        // Vaults first so items never reference a vault the server doesn't know yet.
        Ok((upto, req))
    }

    pub fn sync_apply_push(&mut self, upto: i64, resp: &PushResponse) -> Result<SyncOutcome> {
        let mut out = SyncOutcome::default();
        for (entity, id, version) in &resp.accepted {
            let e = if entity == E_VAULT { E_VAULT } else { E_ITEM };
            self.store.set_hwm(e, *id, *version)?;
            self.store.outbox_clear(upto, e, *id)?;
            out.pushed += 1;
        }
        for v in &resp.conflict_vaults {
            // Vault metadata conflicts are rare (rename on two devices): server wins,
            // our pending change is dropped, the vault key is identical anyway.
            if self.apply_vault(v, true)? {
                self.store.outbox_clear(upto, E_VAULT, v.id)?;
                out.conflicts_resolved += 1;
            } else {
                out.rejected += 1;
            }
        }
        for i in &resp.conflict_items {
            match self.resolve_item(i) {
                Ok(()) => out.conflicts_resolved += 1,
                Err(CoreError::Integrity) => out.rejected += 1,
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    // ---- pull -------------------------------------------------------------------------

    pub fn sync_apply_pull(&mut self, resp: &PullResponse) -> Result<SyncOutcome> {
        let mut out = SyncOutcome::default();
        if let Some(rec) = &resp.record {
            if self.apply_record(rec, resp.record_version).is_err() {
                out.rejected += 1;
            }
        }
        for v in &resp.vaults {
            let hwm = self.store.hwm(E_VAULT, v.id)?;
            if v.version < hwm {
                out.rejected += 1;
                let _ = self.store.log_event("sync.rollback", Some(v.id.as_bytes()));
                continue;
            }
            if v.version == hwm && hwm > 0 {
                continue;
            }
            if self.store.outbox_has(E_VAULT, v.id)? {
                // Our pending rename will be pushed on top of the server version.
                self.store.set_hwm(E_VAULT, v.id, v.version)?;
                continue;
            }
            if self.apply_vault(v, false)? {
                out.pulled += 1;
            } else {
                out.rejected += 1;
            }
        }
        self.apply_pulled_items(&resp.items, &mut out)?;
        self.set_cursor(resp.cursor)?;
        Ok(out)
    }

    /// Items from a pull: rollback check, then apply (or merge with a pending local edit).
    pub(super) fn apply_pulled_items(&mut self, items: &[WireItem], out: &mut SyncOutcome) -> Result<()> {
        for i in items {
            let hwm = self.store.hwm(E_ITEM, i.id)?;
            if i.version < hwm {
                out.rejected += 1;
                let _ = self.store.log_event("sync.rollback", Some(i.id.as_bytes()));
                continue;
            }
            if i.version == hwm && hwm > 0 {
                continue;
            }
            let pending = self.store.outbox_has(E_ITEM, i.id)?;
            let result = if pending { self.resolve_item(i).map(|_| true) } else { self.apply_item(i) };
            match result {
                Ok(true) => {
                    if pending {
                        out.conflicts_resolved += 1;
                    } else {
                        out.pulled += 1;
                    }
                }
                Ok(false) | Err(CoreError::Integrity) => out.rejected += 1,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    // ---- verification -----------------------------------------------------------------

    fn verify_vault(&self, v: &WireVault) -> Result<Verified> {
        let s = self.session()?;
        let acct = s.record.id;
        if v.deleted {
            aead::open(
                &s.ak,
                &v.attrs,
                &Expect { min_version: Some(v.version), ..Expect::purpose(Purpose::Tombstone).account(acct).container(acct).object(v.id) },
            )?;
            return Ok(Verified::Vault(None));
        }
        let (h, key) = aead::unwrap_key(&s.ak, &v.wrapped_key, &Expect::purpose(Purpose::VaultKey).account(acct).container(acct).object(v.id).key(acct))?;
        let (ah, raw) = aead::open(&key, &v.attrs, &Expect::purpose(Purpose::VaultAttrs).account(acct).container(v.id).object(v.id).key(v.id))?;
        if ah.version != v.version {
            return Err(CoreError::Integrity);
        }
        let attrs: VaultAttrs = serde_json::from_slice(&raw)?;
        Ok(Verified::Vault(Some((key, h.version as u32, attrs))))
    }

    fn verify_item(&self, i: &WireItem) -> Result<Verified> {
        let s = self.session()?;
        let vault = s.vaults.get(&i.vault_id).ok_or(CoreError::Integrity)?;
        let acct = vault.owner;
        if i.deleted {
            let (h, _) = aead::open(
                &vault.key,
                &i.overview,
                &Expect::purpose(Purpose::Tombstone).account(acct).container(i.vault_id).object(i.id).key(i.vault_id),
            )?;
            if h.version != i.version {
                return Err(CoreError::Integrity);
            }
            return Ok(Verified::Item(None));
        }
        let item = decrypt_item(acct, &vault.key, &from_wire_item(i))?;
        Ok(Verified::Item(Some(item)))
    }

    // ---- apply ------------------------------------------------------------------------

    /// Returns false when the row failed verification (and was ignored).
    fn apply_vault(&mut self, v: &WireVault, server_wins: bool) -> Result<bool> {
        let verified = match self.verify_vault(v) {
            Ok(Verified::Vault(x)) => x,
            Ok(_) => unreachable!(),
            Err(CoreError::Integrity) => {
                let _ = self.store.log_event("sync.integrity", Some(v.id.as_bytes()));
                return Ok(false);
            }
            Err(e) => return Err(e),
        };
        let _ = server_wins;
        self.store.put_vault_remote(&from_wire_vault(v), v.deleted)?;
        self.store.set_hwm(E_VAULT, v.id, v.version)?;
        let s = self.session_mut()?;
        match verified {
            Some((key, key_gen, attrs)) => {
                s.vaults.insert(
                    v.id,
                    VaultState {
                        owner: s.record.id,
                        role: crate::crypto::share::Role::Owner,
                        key,
                        key_gen,
                        attrs,
                        created_at: v.created_at,
                        updated_at: v.updated_at,
                        revision: v.version,
                    },
                );
            }
            None => {
                s.vaults.remove(&v.id);
                s.items.retain(|_, c| c.vault_id != v.id);
            }
        }
        Ok(true)
    }

    fn apply_item(&mut self, i: &WireItem) -> Result<bool> {
        let verified = match self.verify_item(i) {
            Ok(Verified::Item(x)) => x,
            Ok(_) => unreachable!(),
            Err(CoreError::Integrity) => {
                let _ = self.store.log_event("sync.integrity", Some(i.id.as_bytes()));
                return Ok(false);
            }
            Err(e) => return Err(e),
        };
        // Keep what we had as a history version, so the change is reversible.
        if let (Some((old, false)), false) = (self.store.item_raw(i.id)?, i.deleted) {
            if old.vault_id == i.vault_id {
                self.store.push_history(
                    &HistoryRow {
                        item_id: old.id,
                        vault_id: old.vault_id,
                        revision: old.revision,
                        overview: old.overview,
                        details: old.details,
                        saved_at: old.updated_at,
                    },
                    HISTORY_KEEP,
                )?;
            }
        }
        self.store.put_item_remote(&from_wire_item(i), i.deleted)?;
        self.store.set_hwm(E_ITEM, i.id, i.version)?;
        let s = self.session_mut()?;
        match verified {
            Some(item) => {
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
            None => {
                s.items.remove(&i.id);
            }
        }
        Ok(true)
    }

    /// Both sides changed the same item. The newest edit wins with a fresh version above
    /// both; the other is kept in history. An edit always beats a deletion (nothing lost).
    fn resolve_item(&mut self, server: &WireItem) -> Result<()> {
        let theirs = match self.verify_item(server)? {
            Verified::Item(x) => x,
            _ => unreachable!(),
        };
        let local_raw = self.store.item_raw(server.id)?;
        let s = self.session()?;
        let ours: Option<Item> = match &local_raw {
            Some((row, false)) => {
                let vault = s.vaults.get(&row.vault_id).ok_or(CoreError::Integrity)?;
                Some(decrypt_item(vault.owner, &vault.key, row)?)
            }
            _ => None,
        };
        let local_version = local_raw.as_ref().map(|(r, _)| r.revision).unwrap_or(0);
        let next_version = local_version.max(server.version) + 1;

        let (winner, loser): (Option<Item>, Option<Item>) = match (ours, theirs) {
            (Some(o), Some(t)) => {
                if (o.updated_at, o.revision) >= (t.updated_at, server.version) {
                    (Some(o), Some(t))
                } else {
                    (Some(t), Some(o))
                }
            }
            (Some(o), None) => (Some(o), None), // they deleted, we edited → keep edit
            (None, Some(t)) => (Some(t), None), // we deleted, they edited → keep edit
            (None, None) => (None, None),       // both deleted
        };

        // Base for our next push is the server's version.
        self.store.set_hwm(E_ITEM, server.id, server.version)?;
        match winner {
            None => {
                // Both sides deleted: adopt the server tombstone, nothing to push.
                self.store.put_item_remote(&from_wire_item(server), true)?;
                self.store.outbox_clear(i64::MAX, E_ITEM, server.id)?;
                self.session_mut()?.items.remove(&server.id);
            }
            Some(mut w) => {
                w.revision = next_version;
                w.updated_at = w.updated_at.max(now_ms());
                let s = self.session()?;
                let vault = s.vaults.get(&w.vault_id).ok_or(CoreError::Integrity)?;
                let row = encrypt_item(vault.owner, &vault.key, vault.key_gen, &w)?;
                let history = match loser {
                    Some(l) => {
                        let lv = s.vaults.get(&l.vault_id).ok_or(CoreError::Integrity)?;
                        let enc = encrypt_item(lv.owner, &lv.key, lv.key_gen, &l)?;
                        Some(HistoryRow { item_id: l.id, vault_id: l.vault_id, revision: l.revision, overview: enc.overview, details: enc.details, saved_at: l.updated_at })
                    }
                    None => None,
                };
                self.store.transaction(|st| {
                    st.put_item(&row)?; // enqueues → pushed next round with base = server.version
                    if let Some(h) = &history {
                        st.push_history(h, HISTORY_KEEP)?;
                    }
                    Ok(())
                })?;
                self.cache(&w);
                let _ = self.store.log_event("sync.conflict", Some(server.id.as_bytes()));
            }
        }
        Ok(())
    }
}

/// In-memory server used by tests (and as the reference for the real server's rules).
#[cfg(any(test, feature = "fake-server"))]
pub mod fake {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct FakeServer {
        pub seq: i64,
        pub vaults: HashMap<Uuid, (WireVault, i64)>,
        pub items: HashMap<Uuid, (WireItem, i64)>,
        pub record: Option<(String, u64)>,
    }

    impl FakeServer {
        pub fn push(&mut self, req: &PushRequest) -> PushResponse {
            let mut resp = PushResponse::default();
            for p in &req.vaults {
                let current = self.vaults.get(&p.row.id).map(|(v, _)| v.version).unwrap_or(0);
                if p.base == current && p.row.version > current {
                    self.seq += 1;
                    self.vaults.insert(p.row.id, (p.row.clone(), self.seq));
                    resp.accepted.push((E_VAULT.into(), p.row.id, p.row.version));
                } else if let Some((v, _)) = self.vaults.get(&p.row.id) {
                    resp.conflict_vaults.push(v.clone());
                }
            }
            for p in &req.items {
                let current = self.items.get(&p.row.id).map(|(v, _)| v.version).unwrap_or(0);
                if p.base == current && p.row.version > current {
                    self.seq += 1;
                    self.items.insert(p.row.id, (p.row.clone(), self.seq));
                    resp.accepted.push((E_ITEM.into(), p.row.id, p.row.version));
                } else if let Some((v, _)) = self.items.get(&p.row.id) {
                    resp.conflict_items.push(v.clone());
                }
            }
            resp
        }

        pub fn pull(&self, since: i64, record_version: u64) -> PullResponse {
            let mut vaults: Vec<_> = self.vaults.values().filter(|(_, s)| *s > since).cloned().collect();
            let mut items: Vec<_> = self.items.values().filter(|(_, s)| *s > since).cloned().collect();
            vaults.sort_by_key(|(_, s)| *s);
            items.sort_by_key(|(_, s)| *s);
            let record = self.record.as_ref().filter(|(_, v)| *v > record_version);
            PullResponse {
                cursor: self.seq,
                vaults: vaults.into_iter().map(|(v, _)| v).collect(),
                items: items.into_iter().map(|(v, _)| v).collect(),
                record: record.map(|(r, _)| r.clone()),
                record_version: record.map(|(_, v)| *v).unwrap_or(0),
            }
        }
    }

    /// One full round: push (resolving conflicts), then pull, then push again what the
    /// resolution produced. Mirrors what the desktop does.
    pub fn sync(acct: &mut Account, server: &mut FakeServer) -> Result<SyncOutcome> {
        let mut total = SyncOutcome::default();
        for _ in 0..3 {
            let (upto, req) = acct.sync_collect()?;
            if !req.is_empty() {
                let resp = server.push(&req);
                let o = acct.sync_apply_push(upto, &resp)?;
                total.pushed += o.pushed;
                total.conflicts_resolved += o.conflicts_resolved;
                total.rejected += o.rejected;
            }
            let pull = server.pull(acct.sync_cursor()?, acct.record_version()?);
            let o = acct.sync_apply_pull(&pull)?;
            total.pulled += o.pulled;
            total.conflicts_resolved += o.conflicts_resolved;
            total.rejected += o.rejected;
            if acct.sync_collect()?.1.is_empty() {
                break;
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{sync, FakeServer};
    use super::*;
    use crate::crypto::kdf::KdfParams;
    use crate::crypto::{SecretKey, SecretString};
    use crate::model::{Field, FieldType, ItemInput, ItemKind};
    use crate::store::Store;

    const PW: &str = "correct horse battery";

    fn login(title: &str, pass: &str) -> ItemInput {
        ItemInput {
            kind: ItemKind::Login,
            title: title.into(),
            urls: vec![],
            tags: vec![],
            favorite: false,
            icon: None,
            fields: vec![Field { id: "password".into(), label: String::new(), kind: FieldType::Concealed, value: SecretString::new(pass), section: None }],
            sections: vec![],
            notes: SecretString::default(),
        }
    }

    /// Device A creates the account and syncs; device B signs in with the record.
    fn two_devices() -> (Account, Account, FakeServer, SecretKey) {
        let sk = SecretKey::generate();
        let mut a = Account::new(Store::open_in_memory().unwrap());
        a.upgrade_kdf_on_unlock = false;
        a.create_with_params(PW, &sk, VaultAttrs { name: "Pessoal".into(), description: String::new(), icon: String::new(), color: String::new() }, KdfParams::fast_for_tests())
            .unwrap();
        let mut server = FakeServer::default();
        server.record = Some((a.record_json().unwrap(), 1));
        sync(&mut a, &mut server).unwrap();

        let mut b = Account::new(Store::open_in_memory().unwrap());
        b.upgrade_kdf_on_unlock = false;
        let (rec, v) = server.record.clone().unwrap();
        b.install_record(&rec, a.account_id().unwrap(), v).unwrap();
        b.unlock(PW, &sk).unwrap();
        sync(&mut b, &mut server).unwrap();
        (a, b, server, sk)
    }

    fn vault(a: &Account) -> Uuid {
        a.vaults().unwrap()[0].id
    }

    #[test]
    fn new_device_gets_everything() {
        let (mut a, mut b, mut server, _) = two_devices();
        assert_eq!(b.vaults().unwrap().len(), 1);
        let item = a.create_item(vault(&a), login("GitHub", "s1")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        assert_eq!(b.item(item.id).unwrap().details.value("password"), Some("s1"));
    }

    #[test]
    fn offline_edits_on_both_devices_merge() {
        let (mut a, mut b, mut server, _) = two_devices();
        let x = a.create_item(vault(&a), login("X", "1")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        // Both offline: A edits X, B creates Y.
        a.update_item(x.id, login("X", "2")).unwrap();
        let y = b.create_item(vault(&b), login("Y", "y")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        sync(&mut a, &mut server).unwrap();
        assert_eq!(b.item(x.id).unwrap().details.value("password"), Some("2"));
        assert_eq!(a.item(y.id).unwrap().details.value("password"), Some("y"));
    }

    #[test]
    fn same_item_conflict_keeps_both_versions() {
        let (mut a, mut b, mut server, _) = two_devices();
        let x = a.create_item(vault(&a), login("X", "base")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        a.update_item(x.id, login("X", "from-a")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        b.update_item(x.id, login("X", "from-b")).unwrap(); // newer edit
        sync(&mut a, &mut server).unwrap();
        let out = sync(&mut b, &mut server).unwrap();
        assert!(out.conflicts_resolved >= 1);
        sync(&mut a, &mut server).unwrap();
        for dev in [&a, &b] {
            assert_eq!(dev.item(x.id).unwrap().details.value("password"), Some("from-b"));
        }
        // The losing edit is recoverable from history on the device that resolved it.
        assert!(b.history(x.id).unwrap().iter().any(|v| v.item.details.value("password") == Some("from-a")));
    }

    #[test]
    fn edit_beats_delete() {
        let (mut a, mut b, mut server, _) = two_devices();
        let x = a.create_item(vault(&a), login("X", "1")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        a.purge_item(x.id).unwrap();
        b.update_item(x.id, login("X", "still-here")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        sync(&mut a, &mut server).unwrap();
        assert_eq!(a.item(x.id).unwrap().details.value("password"), Some("still-here"));
        assert_eq!(b.item(x.id).unwrap().details.value("password"), Some("still-here"));
    }

    #[test]
    fn deletes_propagate() {
        let (mut a, mut b, mut server, _) = two_devices();
        let x = a.create_item(vault(&a), login("X", "1")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        a.purge_item(x.id).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        assert!(b.item(x.id).is_err());
        assert!(b.summaries().unwrap().is_empty());
    }

    #[test]
    fn server_rollback_is_rejected() {
        let (mut a, mut b, mut server, _) = two_devices();
        let x = a.create_item(vault(&a), login("X", "old")).unwrap();
        sync(&mut a, &mut server).unwrap();
        let old_row = server.items[&x.id].0.clone();
        a.update_item(x.id, login("X", "new")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        // Malicious server serves the old version again with a fresh sequence number.
        server.seq += 1;
        server.items.insert(x.id, (old_row, server.seq));
        let out = sync(&mut b, &mut server).unwrap();
        assert_eq!(out.rejected, 1);
        assert_eq!(b.item(x.id).unwrap().details.value("password"), Some("new"));
    }

    #[test]
    fn forged_rows_are_rejected() {
        let (mut a, mut b, mut server, _) = two_devices();
        let x = a.create_item(vault(&a), login("X", "1")).unwrap();
        let y = a.create_item(vault(&a), login("Y", "2")).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        // Server swaps Y's ciphertext into X with a higher version number.
        let mut forged = server.items[&y.id].0.clone();
        forged.id = x.id;
        forged.version = 99;
        server.seq += 1;
        server.items.insert(x.id, (forged.clone(), server.seq));
        // And forges a deletion flag without a valid tombstone.
        let mut fake_delete = server.items[&y.id].0.clone();
        fake_delete.deleted = true;
        fake_delete.version = 99;
        server.seq += 1;
        server.items.insert(y.id, (fake_delete, server.seq));
        let out = sync(&mut b, &mut server).unwrap();
        assert_eq!(out.rejected, 2);
        assert_eq!(b.item(x.id).unwrap().details.value("password"), Some("1"));
        assert_eq!(b.item(y.id).unwrap().details.value("password"), Some("2"));
    }

    #[test]
    fn password_change_propagates_record() {
        let (mut a, mut b, mut server, sk) = two_devices();
        a.change_password(PW, "uma senha nova e longa", &sk).unwrap();
        server.record = Some((a.record_json().unwrap(), 2));
        sync(&mut b, &mut server).unwrap();
        b.lock();
        assert!(b.unlock(PW, &sk).is_err());
        b.unlock("uma senha nova e longa", &sk).unwrap();
    }

    #[test]
    fn vault_create_and_delete_sync() {
        let (mut a, mut b, mut server, _) = two_devices();
        let v = a.create_vault(VaultAttrs { name: "Trabalho".into(), description: String::new(), icon: String::new(), color: String::new() }).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        assert!(b.vaults().unwrap().iter().any(|x| x.attrs.name == "Trabalho"));
        a.delete_vault(v.id, None).unwrap();
        sync(&mut a, &mut server).unwrap();
        sync(&mut b, &mut server).unwrap();
        assert!(!b.vaults().unwrap().iter().any(|x| x.id == v.id));
    }
}
