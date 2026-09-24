//! Shared vaults.
//!
//! A vault is shared by giving each member its key, sealed to their identity key and
//! signed by the owner ([`crate::crypto::share`]). Objects in a shared vault keep the
//! owner's account id in their envelope headers, so every member verifies exactly what
//! the owner does. Members sync each shared vault separately (its own cursor), and the
//! server authorizes those reads and writes by membership.
//!
//! The server could try to hand out someone else's public key. Keys are pinned on first
//! use (trust on first use) and a change blocks sharing until the person confirms it;
//! both sides can compare the safety number ([`PublicIdentity::fingerprint`]).
//!
//! Removing a member rotates the vault key, so they can't read anything written after.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::sync::{PullResponse, PushRequest, Push, SyncOutcome, WireItem, WireVault};
use super::{decrypt_item, encrypt_item, Account, VaultAttrs, VaultState};
use crate::crypto::aead::{self, Expect, Header, Purpose};
use crate::crypto::identity::PublicIdentity;
use crate::crypto::share::{self, Role, ShareGrant};
use crate::crypto::SymmetricKey;
use crate::error::{CoreError, Result};
use crate::model::{now_ms, Timestamp};
use crate::store::{HistoryRow, ItemRow, VaultRow};

const CONTACTS_META: &str = "contacts";
const E_ITEM: &str = "item";

/// Someone we share with (or who shares with us), with their pinned public keys.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub account_id: Uuid,
    pub email: String,
    pub identity: PublicIdentity,
    pub pinned_at: Timestamp,
    /// The user compared safety numbers with this person.
    #[serde(default)]
    pub verified: bool,
}

/// Result of checking a public key the server gave us against what we pinned.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KeyCheck {
    /// First time we see this person: now pinned.
    New,
    /// Matches the pinned key.
    Same,
    /// Differs from the pinned key — could be a reinstall, or someone in the middle.
    Changed,
}

/// A vault shared with us, as the server lists it.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SharedEntry {
    pub owner_id: Uuid,
    pub owner_email: String,
    pub owner_identity: PublicIdentity,
    pub grant: ShareGrant,
}

/// A member of one of our vaults, as the server lists it.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub account_id: Uuid,
    pub email: String,
    pub role: Role,
    pub identity: PublicIdentity,
    pub key_gen: u32,
}

fn cursor_key(vault: Uuid) -> String {
    format!("shared-cursor:{vault}")
}

impl Account {
    pub(super) fn is_shared_with_us(&self, vault: Uuid) -> bool {
        self.session.as_ref().and_then(|s| s.vaults.get(&vault)).is_some_and(|v| v.role != Role::Owner)
    }

    /// Our safety number, to read out to someone we share with.
    pub fn my_fingerprint(&self) -> Result<String> {
        Ok(self.session()?.record.public_identity.fingerprint())
    }

    // ---- contacts (pinned keys) -------------------------------------------------------

    pub fn contacts(&self) -> Result<Vec<Contact>> {
        let s = self.session()?;
        let acct = s.record.id;
        let Some(blob) = self.store.meta_get(CONTACTS_META)? else { return Ok(Vec::new()) };
        let (_, raw) = aead::open(&s.ak, &blob, &Expect::purpose(Purpose::Contacts).account(acct).object(acct))?;
        Ok(serde_json::from_slice(&raw)?)
    }

    fn save_contacts(&self, contacts: &[Contact]) -> Result<()> {
        let s = self.session()?;
        let acct = s.record.id;
        let h = Header::new(Purpose::Contacts, acct).container(acct).object(acct).key(acct, s.record.ak_gen).padded();
        self.store.meta_set(CONTACTS_META, &aead::seal(&s.ak, &h, &serde_json::to_vec(contacts)?))
    }

    /// Checks a public key against the pinned one; pins it the first time.
    pub fn check_contact(&self, account_id: Uuid, email: &str, identity: &PublicIdentity) -> Result<KeyCheck> {
        if account_id == self.session()?.record.id {
            return Err(CoreError::Invalid("esse é o seu próprio e-mail".into()));
        }
        let mut contacts = self.contacts()?;
        match contacts.iter_mut().find(|c| c.account_id == account_id) {
            Some(c) if &c.identity == identity => {
                if c.email != email {
                    c.email = email.to_string();
                    self.save_contacts(&contacts)?;
                }
                Ok(KeyCheck::Same)
            }
            Some(_) => Ok(KeyCheck::Changed),
            None => {
                contacts.push(Contact {
                    account_id,
                    email: email.to_string(),
                    identity: identity.clone(),
                    pinned_at: now_ms(),
                    verified: false,
                });
                self.save_contacts(&contacts)?;
                Ok(KeyCheck::New)
            }
        }
    }

    /// The user confirmed a changed key (e.g. the person reinstalled): pin the new one.
    pub fn trust_new_key(&self, account_id: Uuid, identity: &PublicIdentity) -> Result<()> {
        let mut contacts = self.contacts()?;
        let c = contacts.iter_mut().find(|c| c.account_id == account_id).ok_or_else(|| CoreError::NotFound("contato".into()))?;
        c.identity = identity.clone();
        c.pinned_at = now_ms();
        c.verified = false;
        self.save_contacts(&contacts)
    }

    pub fn set_contact_verified(&self, account_id: Uuid, verified: bool) -> Result<()> {
        let mut contacts = self.contacts()?;
        let c = contacts.iter_mut().find(|c| c.account_id == account_id).ok_or_else(|| CoreError::NotFound("contato".into()))?;
        c.verified = verified;
        self.save_contacts(&contacts)
    }

    fn pinned(&self, account_id: Uuid, identity: &PublicIdentity) -> Result<()> {
        match self.contacts()?.iter().find(|c| c.account_id == account_id) {
            Some(c) if &c.identity == identity => Ok(()),
            Some(_) => Err(CoreError::Conflict("a chave de segurança dessa pessoa mudou".into())),
            None => Err(CoreError::NotFound("contato".into())),
        }
    }

    // ---- owner side -------------------------------------------------------------------

    /// Seals one of our vaults' key to a member whose key is pinned.
    pub fn grant_vault(&self, vault_id: Uuid, member_id: Uuid, member: &PublicIdentity, role: Role) -> Result<ShareGrant> {
        if role == Role::Owner {
            return Err(CoreError::Invalid("papel inválido".into()));
        }
        self.pinned(member_id, member)?;
        let s = self.session()?;
        let v = s.vaults.get(&vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        if v.role != Role::Owner {
            return Err(CoreError::Invalid("só quem criou o cofre pode compartilhá-lo".into()));
        }
        share::grant(&s.identity, s.record.id, vault_id, v.key_gen, &v.key, member_id, member, role)
    }

    pub fn vault_key_gen(&self, vault_id: Uuid) -> Result<u32> {
        Ok(self.session()?.vaults.get(&vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?.key_gen)
    }

    /// New key for one of our vaults (after removing a member): every item, its history
    /// and the vault attributes are re-encrypted and queued for sync. Remaining members
    /// need fresh grants ([`Account::grant_vault`]).
    pub fn rotate_vault_key(&mut self, vault_id: Uuid) -> Result<u32> {
        let s = self.session()?;
        let old = s.vaults.get(&vault_id).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        if old.role != Role::Owner {
            return Err(CoreError::Invalid("só quem criou o cofre pode trocar a chave".into()));
        }
        let owner = old.owner;
        let new_key = SymmetricKey::generate();
        let new_gen = old.key_gen + 1;
        let now = now_ms();

        let mut rows = Vec::new();
        let mut items = Vec::new();
        for row in self.store.items_in_vault(vault_id)? {
            let mut item = decrypt_item(owner, &old.key, &row)?;
            item.revision += 1;
            item.updated_at = item.updated_at.max(now);
            rows.push(encrypt_item(owner, &new_key, new_gen, &item)?);
            items.push(item);
        }
        let history = self.reencrypt_history(vault_id, owner, &old.key, &new_key, new_gen)?;
        let attachments = self.reencrypt_attachments(vault_id, owner, &old.key, &new_key, new_gen)?;

        let state = VaultState {
            owner,
            role: Role::Owner,
            key: new_key,
            key_gen: new_gen,
            attrs: old.attrs.clone(),
            created_at: old.created_at,
            updated_at: now,
            revision: old.revision + 1,
        };
        let vrow = self.vault_row(vault_id, &state, &state.attrs.clone(), state.revision, now)?;
        self.store.transaction(|st| {
            st.put_vault(&vrow)?;
            for r in &rows {
                st.put_item(r)?;
            }
            for (item_id, h) in &history {
                st.replace_history(*item_id, h)?;
            }
            for (att, item, blob) in &attachments {
                // New ciphertext: upload again.
                st.put_attachment(*att, *item, vault_id, blob, now)?;
            }
            Ok(())
        })?;
        self.session_mut()?.vaults.insert(vault_id, state);
        for item in &items {
            self.cache(item);
        }
        let _ = self.store.log_event("share.rotate", Some(vault_id.as_bytes()));
        Ok(new_gen)
    }

    fn reencrypt_history(
        &self,
        vault_id: Uuid,
        owner: Uuid,
        old: &SymmetricKey,
        new: &SymmetricKey,
        new_gen: u32,
    ) -> Result<Vec<(Uuid, Vec<HistoryRow>)>> {
        let mut out = Vec::new();
        for row in self.store.items_in_vault(vault_id)? {
            let mut rows = Vec::new();
            for h in self.store.history(row.id)? {
                let r = ItemRow {
                    id: h.item_id,
                    vault_id: h.vault_id,
                    revision: h.revision,
                    overview: h.overview,
                    details: h.details,
                    created_at: 0,
                    updated_at: h.saved_at,
                };
                if let Ok(item) = decrypt_item(owner, old, &r) {
                    let enc = encrypt_item(owner, new, new_gen, &item)?;
                    rows.push(HistoryRow {
                        item_id: h.item_id,
                        vault_id,
                        revision: h.revision,
                        overview: enc.overview,
                        details: enc.details,
                        saved_at: h.saved_at,
                    });
                }
            }
            out.push((row.id, rows));
        }
        Ok(out)
    }

    fn reencrypt_attachments(
        &self,
        vault_id: Uuid,
        owner: Uuid,
        old: &SymmetricKey,
        new: &SymmetricKey,
        new_gen: u32,
    ) -> Result<Vec<(Uuid, Uuid, Vec<u8>)>> {
        let mut out = Vec::new();
        for row in self.store.items_in_vault(vault_id)? {
            for (att, _, blob) in self.store.attachments_of(row.id)? {
                let expect = Expect::purpose(Purpose::Attachment).account(owner).container(row.id).object(att).key(vault_id);
                let (_, pt) = aead::open(old, &blob, &expect)?;
                let h = Header::new(Purpose::Attachment, owner).container(row.id).object(att).key(vault_id, new_gen).padded();
                out.push((att, row.id, aead::seal(new, &h, &pt)));
            }
        }
        Ok(out)
    }

    // ---- member side ------------------------------------------------------------------

    /// Vaults shared with us that we hold locally: (vault, owner).
    pub fn shared_vault_ids(&self) -> Result<Vec<(Uuid, Uuid)>> {
        Ok(self.session()?.vaults.iter().filter(|(_, v)| v.role != Role::Owner).map(|(id, v)| (*id, v.owner)).collect())
    }

    pub fn shared_cursor(&self, vault: Uuid) -> Result<i64> {
        Ok(self
            .store
            .meta_get(&cursor_key(vault))?
            .and_then(|v| String::from_utf8(v).ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0))
    }

    /// We left the vault or were removed: forget it and everything in it.
    pub fn forget_shared_vault(&mut self, vault: Uuid) -> Result<()> {
        if !self.is_shared_with_us(vault) {
            return Err(CoreError::NotFound("cofre compartilhado".into()));
        }
        self.store.remove_shared_vault(vault)?;
        let s = self.session_mut()?;
        s.vaults.remove(&vault);
        let gone: Vec<Uuid> = s.items.iter().filter(|(_, c)| c.vault_id == vault).map(|(id, _)| *id).collect();
        for id in gone {
            s.items.remove(&id);
            s.local.usage.remove(&id);
        }
        let _ = self.store.log_event("share.left", Some(vault.as_bytes()));
        self.save_local()
    }

    /// Applies one shared vault from the server: verifies the owner's grant (and pinned
    /// key), installs or updates the vault, then the pulled items.
    pub fn sync_shared_pull(&mut self, entry: &SharedEntry, resp: &PullResponse) -> Result<SyncOutcome> {
        let g = &entry.grant;
        let my_id = self.session()?.record.id;
        if g.owner_id != entry.owner_id || g.member_id != my_id {
            return Err(CoreError::Integrity);
        }
        // Pin the owner on first sight; a changed key blocks this vault until confirmed.
        match self.check_contact(entry.owner_id, &entry.owner_email, &entry.owner_identity)? {
            KeyCheck::Changed => return Err(CoreError::Conflict("a chave de segurança de quem compartilhou mudou".into())),
            KeyCheck::New | KeyCheck::Same => {}
        }
        let vault_row = resp.vaults.iter().find(|v| v.id == g.vault_id).ok_or(CoreError::Integrity)?;
        if vault_row.deleted {
            if self.is_shared_with_us(g.vault_id) {
                self.forget_shared_vault(g.vault_id)?;
            }
            return Ok(SyncOutcome::default());
        }
        self.install_shared_vault(entry, vault_row)?;

        let mut out = SyncOutcome::default();
        let items: Vec<WireItem> = resp.items.iter().filter(|i| i.vault_id == g.vault_id).cloned().collect();
        out.rejected += resp.items.len() - items.len();
        self.apply_pulled_items(&items, &mut out)?;
        self.store.meta_set(&cursor_key(g.vault_id), resp.cursor.to_string().as_bytes())?;
        Ok(out)
    }

    fn install_shared_vault(&mut self, entry: &SharedEntry, row: &WireVault) -> Result<()> {
        let g = &entry.grant;
        let s = self.session()?;
        let acct = s.record.id;
        let current = s.vaults.get(&g.vault_id);
        if let Some(c) = current {
            if c.role == Role::Owner || c.owner != g.owner_id {
                return Err(CoreError::Integrity);
            }
            if g.key_gen < c.key_gen || row.version < c.revision {
                return Err(CoreError::Integrity); // rollback
            }
            if g.key_gen == c.key_gen && row.version == c.revision && g.role == c.role {
                return Ok(());
            }
        }
        let key = share::open(g, &entry.owner_identity, &s.identity, acct)?;
        let (ah, raw) = aead::open(
            &key,
            &row.attrs,
            &Expect::purpose(Purpose::VaultAttrs).account(g.owner_id).container(g.vault_id).object(g.vault_id).key(g.vault_id),
        )?;
        if ah.version != row.version {
            return Err(CoreError::Integrity);
        }
        let attrs: VaultAttrs = serde_json::from_slice(&raw)?;
        let local_wrap = aead::wrap_key(
            &s.ak,
            &Header::new(Purpose::VaultKey, acct).container(g.owner_id).object(g.vault_id).key(acct, s.record.ak_gen).version(g.key_gen as u64),
            &key,
        );
        let old_key = current.filter(|c| c.key_gen != g.key_gen).map(|c| SymmetricKey::from_bytes(*c.key.as_bytes()));
        let created_at = current.map(|c| c.created_at).unwrap_or(row.created_at);
        self.store.put_shared_vault(
            &VaultRow { id: g.vault_id, wrapped_key: local_wrap, attrs: row.attrs.clone(), created_at, updated_at: row.updated_at, revision: row.version },
            g.owner_id,
            g.role,
        )?;
        if let Some(old) = old_key {
            self.rekey_local_copies(g.vault_id, g.owner_id, &old, &key, g.key_gen)?;
        }
        self.session_mut()?.vaults.insert(
            g.vault_id,
            VaultState {
                owner: g.owner_id,
                role: g.role,
                key,
                key_gen: g.key_gen,
                attrs,
                created_at,
                updated_at: row.updated_at,
                revision: row.version,
            },
        );
        Ok(())
    }

    /// The owner rotated the key: our unsent edits and local history move to the new key
    /// (same versions — they're still the same writes).
    fn rekey_local_copies(&mut self, vault: Uuid, owner: Uuid, old: &SymmetricKey, new: &SymmetricKey, gen: u32) -> Result<()> {
        let (_, pending) = self.store.outbox_items_in_vault(vault)?;
        for id in pending {
            if let Some((row, false)) = self.store.item_raw(id)? {
                if let Ok(item) = decrypt_item(owner, old, &row) {
                    self.store.put_item_remote(&encrypt_item(owner, new, gen, &item)?, false)?;
                }
            }
        }
        for (item_id, rows) in self.reencrypt_history(vault, owner, old, new, gen)? {
            self.store.replace_history(item_id, &rows)?;
        }
        for (att, item, blob) in self.reencrypt_attachments(vault, owner, old, new, gen).unwrap_or_default() {
            self.store.put_attachment_state(att, item, vault, &blob, now_ms(), true)?;
        }
        Ok(())
    }

    /// Our unsent changes to items of one shared vault.
    pub fn shared_collect(&self, vault: Uuid) -> Result<(i64, PushRequest)> {
        let s = self.session()?;
        let v = s.vaults.get(&vault).ok_or_else(|| CoreError::NotFound("cofre".into()))?;
        let mut req = PushRequest::default();
        if !v.role.can_write() {
            return Ok((0, req));
        }
        let (upto, ids) = self.store.outbox_items_in_vault(vault)?;
        for id in ids {
            if let Some((row, deleted)) = self.store.item_raw(id)? {
                let base = self.store.hwm(E_ITEM, id)?;
                if row.revision > base {
                    req.items.push(Push { base, row: super::sync::wire_item(&row, deleted) });
                }
            }
        }
        Ok((upto, req))
    }
}

/// In-memory stand-in for the server's sharing endpoints (tests, and the reference for
/// the real server's rules).
#[cfg(any(test, feature = "fake-server"))]
pub mod fake {
    use super::*;
    use crate::account::sync::fake::FakeServer;
    use crate::account::sync::PushResponse;
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct FakeCloud {
        pub accounts: HashMap<Uuid, FakeServer>,
        pub emails: HashMap<Uuid, String>,
        pub identities: HashMap<Uuid, PublicIdentity>,
        /// (owner, vault, member) → grant
        pub members: HashMap<(Uuid, Uuid, Uuid), ShareGrant>,
    }

    /// Key generation an envelope was sealed with (plaintext header bytes 68..72).
    pub fn key_gen_of(envelope: &[u8]) -> u32 {
        envelope.get(68..72).map(|b| u32::from_be_bytes(b.try_into().unwrap())).unwrap_or(0)
    }

    impl FakeCloud {
        pub fn register(&mut self, acct: &Account, email: &str) {
            let id = acct.account_id().unwrap();
            self.accounts.entry(id).or_default();
            self.emails.insert(id, email.into());
            self.identities.insert(id, acct.public_identity().unwrap());
        }

        pub fn lookup(&self, email: &str) -> Option<(Uuid, PublicIdentity)> {
            let (id, _) = self.emails.iter().find(|(_, e)| e.as_str() == email)?;
            Some((*id, self.identities[id].clone()))
        }

        pub fn add_member(&mut self, owner: Uuid, g: ShareGrant) {
            assert_eq!(g.owner_id, owner);
            self.members.insert((owner, g.vault_id, g.member_id), g);
        }

        pub fn remove_member(&mut self, owner: Uuid, vault: Uuid, member: Uuid) {
            self.members.remove(&(owner, vault, member));
        }

        pub fn members_of(&self, owner: Uuid, vault: Uuid) -> Vec<Uuid> {
            self.members.keys().filter(|(o, v, _)| *o == owner && *v == vault).map(|(_, _, m)| *m).collect()
        }

        pub fn shared_with(&self, member: Uuid) -> Vec<SharedEntry> {
            self.members
                .iter()
                .filter(|((_, _, m), _)| *m == member)
                .map(|((o, _, _), g)| SharedEntry {
                    owner_id: *o,
                    owner_email: self.emails[o].clone(),
                    owner_identity: self.identities[o].clone(),
                    grant: g.clone(),
                })
                .collect()
        }

        pub fn shared_pull(&self, member: Uuid, owner: Uuid, vault: Uuid, since: i64) -> Option<PullResponse> {
            self.members.get(&(owner, vault, member))?;
            let srv = &self.accounts[&owner];
            let mut items: Vec<_> = srv.items.values().filter(|(i, s)| i.vault_id == vault && *s > since).cloned().collect();
            items.sort_by_key(|(_, s)| *s);
            Some(PullResponse {
                cursor: srv.seq,
                vaults: srv.vaults.get(&vault).map(|(v, _)| v.clone()).into_iter().collect(),
                items: items.into_iter().map(|(i, _)| i).collect(),
                record: None,
                record_version: 0,
            })
        }

        /// Editors only; rows must stay in this vault and use the current key.
        pub fn shared_push(&mut self, member: Uuid, owner: Uuid, vault: Uuid, req: &PushRequest) -> Option<PushResponse> {
            let g = self.members.get(&(owner, vault, member))?;
            if !g.role.can_write() {
                return None;
            }
            let gen = g.key_gen;
            let srv = self.accounts.get_mut(&owner)?;
            for p in &req.items {
                let stays = srv.items.get(&p.row.id).is_none_or(|(i, _)| i.vault_id == vault);
                if p.row.vault_id != vault || !stays || key_gen_of(&p.row.overview) < gen {
                    return None;
                }
            }
            Some(srv.push(&PushRequest { vaults: vec![], items: req.items.clone() }))
        }
    }

    /// Own-account sync plus every shared vault (what the desktop loop does).
    pub fn sync_all(acct: &mut Account, cloud: &mut FakeCloud) -> Result<SyncOutcome> {
        let me = acct.account_id()?;
        let mut out = crate::account::sync::fake::sync(acct, cloud.accounts.get_mut(&me).unwrap())?;
        let entries = cloud.shared_with(me);
        for (vault, _) in acct.shared_vault_ids()? {
            if !entries.iter().any(|e| e.grant.vault_id == vault) {
                acct.forget_shared_vault(vault)?;
            }
        }
        for e in entries {
            let (owner, vault) = (e.owner_id, e.grant.vault_id);
            if acct.is_shared_with_us(vault) {
                let (upto, req) = acct.shared_collect(vault)?;
                if !req.is_empty() {
                    if let Some(resp) = cloud.shared_push(me, owner, vault, &req) {
                        let o = acct.sync_apply_push(upto, &resp)?;
                        out.pushed += o.pushed;
                    }
                }
            }
            let since = acct.shared_cursor(vault)?;
            if let Some(pull) = cloud.shared_pull(me, owner, vault, since) {
                let o = acct.sync_shared_pull(&e, &pull)?;
                out.pulled += o.pulled;
                out.rejected += o.rejected;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{sync_all, FakeCloud};
    use super::*;
    use crate::crypto::identity::IdentityKeys;
    use crate::crypto::kdf::KdfParams;
    use crate::crypto::{SecretKey, SecretString};
    use crate::model::{Field, FieldType, ItemInput, ItemKind};
    use crate::store::Store;

    const PW: &str = "correct horse battery";

    fn person(cloud: &mut FakeCloud, email: &str) -> (Account, SecretKey) {
        let mut a = Account::new(Store::open_in_memory().unwrap());
        a.upgrade_kdf_on_unlock = false;
        let sk = SecretKey::generate();
        a.create_with_params(
            PW,
            &sk,
            VaultAttrs { name: "Pessoal".into(), description: String::new(), icon: String::new(), color: String::new() },
            KdfParams::fast_for_tests(),
        )
        .unwrap();
        cloud.register(&a, email);
        (a, sk)
    }

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

    fn family_vault(a: &mut Account) -> Uuid {
        a.create_vault(VaultAttrs { name: "Família".into(), description: String::new(), icon: "users".into(), color: "moss".into() }).unwrap().id
    }

    /// Owner looks the member up, pins the key and grants access.
    fn share(cloud: &mut FakeCloud, owner: &Account, vault: Uuid, email: &str, role: Role) -> Uuid {
        let (mid, ident) = cloud.lookup(email).unwrap();
        assert_ne!(owner.check_contact(mid, email, &ident).unwrap(), KeyCheck::Changed);
        let g = owner.grant_vault(vault, mid, &ident, role).unwrap();
        cloud.add_member(owner.account_id().unwrap(), g);
        mid
    }

    fn titles(a: &Account, vault: Uuid) -> Vec<String> {
        let mut t: Vec<String> =
            a.summaries().unwrap().into_iter().filter(|s| s.vault_id == vault).map(|s| s.title).collect();
        t.sort();
        t
    }

    fn password(a: &Account, id: Uuid) -> String {
        a.item(id).unwrap().details.fields[0].value.expose().to_string()
    }

    #[test]
    fn editor_sees_and_edits_owner_sees_edit() {
        let mut cloud = FakeCloud::default();
        let (mut ana, _) = person(&mut cloud, "ana@ex.com");
        let (mut beto, beto_sk) = person(&mut cloud, "beto@ex.com");
        let fam = family_vault(&mut ana);
        let netflix = ana.create_item(fam, login("Netflix", "n1")).unwrap();
        sync_all(&mut ana, &mut cloud).unwrap();
        share(&mut cloud, &ana, fam, "beto@ex.com", Role::Editor);

        sync_all(&mut beto, &mut cloud).unwrap();
        let v = beto.vaults().unwrap().into_iter().find(|v| v.id == fam).unwrap();
        assert_eq!(v.attrs.name, "Família");
        assert_eq!(v.role, Role::Editor);
        assert_eq!(v.owner_id, Some(ana.account_id().unwrap()));
        assert_eq!(password(&beto, netflix.id), "n1");

        beto.update_item(netflix.id, login("Netflix", "n2")).unwrap();
        beto.create_item(fam, login("Wi-Fi casa", "w")).unwrap();
        sync_all(&mut beto, &mut cloud).unwrap();
        // Nothing of the shared vault landed in Beto's own account.
        let beto_srv = &cloud.accounts[&beto.account_id().unwrap()];
        assert!(beto_srv.items.values().all(|(i, _)| i.vault_id != fam));

        sync_all(&mut ana, &mut cloud).unwrap();
        assert_eq!(password(&ana, netflix.id), "n2");
        assert_eq!(titles(&ana, fam), vec!["Netflix", "Wi-Fi casa"]);
        // The shared vault survives a relock on the member side.
        beto.lock();
        beto.unlock(PW, &beto_sk).unwrap();
        assert_eq!(titles(&beto, fam), vec!["Netflix", "Wi-Fi casa"]);
        assert_eq!(password(&beto, netflix.id), "n2");
    }

    #[test]
    fn reader_cannot_write_locally_or_on_server() {
        let mut cloud = FakeCloud::default();
        let (mut ana, _) = person(&mut cloud, "ana@ex.com");
        let (mut caio, _) = person(&mut cloud, "caio@ex.com");
        let fam = family_vault(&mut ana);
        let item = ana.create_item(fam, login("Banco", "b")).unwrap();
        sync_all(&mut ana, &mut cloud).unwrap();
        share(&mut cloud, &ana, fam, "caio@ex.com", Role::Reader);
        sync_all(&mut caio, &mut cloud).unwrap();
        assert!(caio.item(item.id).is_ok());
        assert!(caio.update_item(item.id, login("Banco", "x")).is_err());
        assert!(caio.create_item(fam, login("Novo", "x")).is_err());
        assert!(caio.purge_item(item.id).is_err());
        // Even a hand-built push is refused by the server.
        let req = PushRequest::default();
        assert!(cloud.shared_push(caio.account_id().unwrap(), ana.account_id().unwrap(), fam, &req).is_none());
    }

    #[test]
    fn substituted_keys_are_caught_by_pinning() {
        let mut cloud = FakeCloud::default();
        let (mut ana, _) = person(&mut cloud, "ana@ex.com");
        let (mut beto, _) = person(&mut cloud, "beto@ex.com");
        let fam = family_vault(&mut ana);
        sync_all(&mut ana, &mut cloud).unwrap();
        let bid = share(&mut cloud, &ana, fam, "beto@ex.com", Role::Editor);
        sync_all(&mut beto, &mut cloud).unwrap();

        // The server now claims Beto has another key: Ana won't seal anything to it.
        let mallory = IdentityKeys::generate().public();
        assert_eq!(ana.check_contact(bid, "beto@ex.com", &mallory).unwrap(), KeyCheck::Changed);
        assert!(ana.grant_vault(fam, bid, &mallory, Role::Editor).is_err());

        // And claims Ana has another key: Beto refuses the vault's updates.
        let aid = ana.account_id().unwrap();
        cloud.identities.insert(aid, mallory);
        let e = cloud.shared_with(bid).remove(0);
        let pull = cloud.shared_pull(bid, aid, fam, 0).unwrap();
        assert!(matches!(beto.sync_shared_pull(&e, &pull), Err(CoreError::Conflict(_))));
    }

    #[test]
    fn tampered_grants_are_rejected() {
        let mut cloud = FakeCloud::default();
        let (mut ana, _) = person(&mut cloud, "ana@ex.com");
        let (mut beto, _) = person(&mut cloud, "beto@ex.com");
        let fam = family_vault(&mut ana);
        sync_all(&mut ana, &mut cloud).unwrap();
        let bid = share(&mut cloud, &ana, fam, "beto@ex.com", Role::Reader);
        let aid = ana.account_id().unwrap();
        // Server upgrades Beto to editor without Ana's signature.
        cloud.members.get_mut(&(aid, fam, bid)).unwrap().role = Role::Editor;
        let e = cloud.shared_with(bid).remove(0);
        let pull = cloud.shared_pull(bid, aid, fam, 0).unwrap();
        assert!(beto.sync_shared_pull(&e, &pull).is_err());
        assert!(beto.vaults().unwrap().iter().all(|v| v.id != fam));
    }

    #[test]
    fn removed_member_loses_access_to_new_writes() {
        let mut cloud = FakeCloud::default();
        let (mut ana, ana_sk) = person(&mut cloud, "ana@ex.com");
        let (mut beto, _) = person(&mut cloud, "beto@ex.com");
        let (mut caio, _) = person(&mut cloud, "caio@ex.com");
        let fam = family_vault(&mut ana);
        ana.create_item(fam, login("Antigo", "a")).unwrap();
        sync_all(&mut ana, &mut cloud).unwrap();
        let bid = share(&mut cloud, &ana, fam, "beto@ex.com", Role::Editor);
        share(&mut cloud, &ana, fam, "caio@ex.com", Role::Editor);
        sync_all(&mut beto, &mut cloud).unwrap();
        sync_all(&mut caio, &mut cloud).unwrap();
        // Beto keeps the old grant around, as a malicious ex-member would.
        let old_entry = cloud.shared_with(bid).remove(0);
        let aid = ana.account_id().unwrap();

        // Ana removes Beto: key rotation, fresh grant for Caio.
        cloud.remove_member(aid, fam, bid);
        let gen = ana.rotate_vault_key(fam).unwrap();
        assert_eq!(gen, 2);
        for m in cloud.members_of(aid, fam) {
            let ident = cloud.identities[&m].clone();
            let role = cloud.members[&(aid, fam, m)].role;
            let g = ana.grant_vault(fam, m, &ident, role).unwrap();
            cloud.add_member(aid, g);
        }
        ana.create_item(fam, login("Novo segredo", "s")).unwrap();
        sync_all(&mut ana, &mut cloud).unwrap();

        // Beto's next sync drops the vault.
        sync_all(&mut beto, &mut cloud).unwrap();
        assert!(beto.vaults().unwrap().iter().all(|v| v.id != fam));
        // Even handed the new rows directly, his old key can't open them.
        let srv = &cloud.accounts[&aid];
        let leaked = PullResponse {
            cursor: srv.seq,
            vaults: vec![srv.vaults[&fam].0.clone()],
            items: srv.items.values().map(|(i, _)| i.clone()).collect(),
            record: None,
            record_version: 0,
        };
        assert!(beto.sync_shared_pull(&old_entry, &leaked).is_err());

        // Caio follows the rotation and reads both.
        sync_all(&mut caio, &mut cloud).unwrap();
        assert_eq!(titles(&caio, fam), vec!["Antigo", "Novo segredo"]);
        // And the owner still opens everything after a relock.
        ana.lock();
        ana.unlock(PW, &ana_sk).unwrap();
        assert_eq!(titles(&ana, fam), vec!["Antigo", "Novo segredo"]);
    }

    #[test]
    fn member_edit_with_old_key_after_rotation_is_reencrypted() {
        let mut cloud = FakeCloud::default();
        let (mut ana, _) = person(&mut cloud, "ana@ex.com");
        let (mut beto, _) = person(&mut cloud, "beto@ex.com");
        let fam = family_vault(&mut ana);
        let item = ana.create_item(fam, login("Luz", "1")).unwrap();
        sync_all(&mut ana, &mut cloud).unwrap();
        let bid = share(&mut cloud, &ana, fam, "beto@ex.com", Role::Editor);
        sync_all(&mut beto, &mut cloud).unwrap();
        // Beto edits offline while Ana rotates (e.g. after removing someone else).
        beto.create_item(fam, login("Água", "2")).unwrap();
        let aid = ana.account_id().unwrap();
        ana.rotate_vault_key(fam).unwrap();
        let ident = cloud.identities[&bid].clone();
        let g = ana.grant_vault(fam, bid, &ident, Role::Editor).unwrap();
        cloud.add_member(aid, g);
        sync_all(&mut ana, &mut cloud).unwrap();
        // First round: the old-key push is refused; the pull installs the new key and
        // re-encrypts the pending edit; second round pushes it.
        sync_all(&mut beto, &mut cloud).unwrap();
        sync_all(&mut beto, &mut cloud).unwrap();
        sync_all(&mut ana, &mut cloud).unwrap();
        assert_eq!(titles(&ana, fam), vec!["Luz", "Água"]);
        assert!(ana.item(item.id).is_ok());
    }

    #[test]
    fn leaving_forgets_everything_and_cross_owner_move_copies() {
        let mut cloud = FakeCloud::default();
        let (mut ana, _) = person(&mut cloud, "ana@ex.com");
        let (mut beto, _) = person(&mut cloud, "beto@ex.com");
        let fam = family_vault(&mut ana);
        sync_all(&mut ana, &mut cloud).unwrap();
        let bid = share(&mut cloud, &ana, fam, "beto@ex.com", Role::Editor);
        sync_all(&mut beto, &mut cloud).unwrap();

        // Beto moves one of his own items into the family vault.
        let own = beto.vaults().unwrap().into_iter().find(|v| v.role == Role::Owner).unwrap().id;
        let mine = beto.create_item(own, login("Spotify", "s")).unwrap();
        let moved = beto.move_item(mine.id, fam).unwrap();
        assert_ne!(moved.id, mine.id);
        assert!(beto.item(mine.id).is_err());
        sync_all(&mut beto, &mut cloud).unwrap();
        sync_all(&mut ana, &mut cloud).unwrap();
        assert_eq!(titles(&ana, fam), vec!["Spotify"]);

        // Beto leaves.
        cloud.remove_member(ana.account_id().unwrap(), fam, bid);
        beto.forget_shared_vault(fam).unwrap();
        assert!(beto.summaries().unwrap().iter().all(|s| s.vault_id != fam));
        assert!(beto.store.items_in_vault(fam).unwrap().is_empty());
        // His own vault still works and syncs.
        beto.create_item(own, login("Depois", "d")).unwrap();
        sync_all(&mut beto, &mut cloud).unwrap();
    }
}
