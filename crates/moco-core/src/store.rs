//! Encrypted-at-rest local storage (SQLite). This layer only moves opaque ciphertext
//! around; it never sees keys or plaintext. Plaintext columns are limited to ids,
//! revisions, timestamps and tombstones (DECISIONS.md D-006).

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::model::Timestamp;

const SCHEMA_VERSION: i64 = 4;

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Storage(e.to_string())
    }
}

pub struct Store {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct VaultRow {
    pub id: Uuid,
    pub wrapped_key: Vec<u8>,
    pub attrs: Vec<u8>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub revision: u64,
}

#[derive(Debug, Clone)]
pub struct ItemRow {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub revision: u64,
    pub overview: Vec<u8>,
    pub details: Vec<u8>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub item_id: Uuid,
    pub vault_id: Uuid,
    pub revision: u64,
    pub overview: Vec<u8>,
    pub details: Vec<u8>,
    pub saved_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Vault,
    Item,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Upsert,
    Delete,
}

fn uuid_of(s: String) -> rusqlite::Result<Uuid> {
    Uuid::parse_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // FULL sync: a password manager must never lose a committed write on power loss.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;
             PRAGMA secure_delete = ON;
             PRAGMA temp_store = MEMORY;",
        )?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let version: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(CoreError::Unsupported(format!(
                "este cofre foi criado por uma versão mais nova do Mocó (esquema {version})"
            )));
        }
        if version < 1 {
            self.conn.execute_batch(
                "BEGIN;
                 CREATE TABLE meta (
                   key   TEXT PRIMARY KEY,
                   value BLOB NOT NULL
                 );
                 CREATE TABLE vaults (
                   id          TEXT PRIMARY KEY,
                   wrapped_key BLOB NOT NULL,
                   attrs       BLOB NOT NULL,
                   created_at  INTEGER NOT NULL,
                   updated_at  INTEGER NOT NULL,
                   revision    INTEGER NOT NULL,
                   deleted     INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE items (
                   id         TEXT PRIMARY KEY,
                   vault_id   TEXT NOT NULL REFERENCES vaults(id),
                   revision   INTEGER NOT NULL,
                   overview   BLOB,
                   details    BLOB,
                   created_at INTEGER NOT NULL,
                   updated_at INTEGER NOT NULL,
                   deleted    INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE INDEX items_vault ON items(vault_id);
                 CREATE TABLE item_history (
                   item_id   TEXT NOT NULL,
                   revision  INTEGER NOT NULL,
                   vault_id  TEXT NOT NULL,
                   overview  BLOB NOT NULL,
                   details   BLOB NOT NULL,
                   saved_at  INTEGER NOT NULL,
                   PRIMARY KEY (item_id, revision)
                 );
                 CREATE TABLE outbox (
                   seq       INTEGER PRIMARY KEY AUTOINCREMENT,
                   entity    TEXT NOT NULL,
                   entity_id TEXT NOT NULL,
                   revision  INTEGER NOT NULL,
                   op        TEXT NOT NULL,
                   at        INTEGER NOT NULL
                 );
                 CREATE TABLE events (
                   id     INTEGER PRIMARY KEY AUTOINCREMENT,
                   at     INTEGER NOT NULL,
                   kind   TEXT NOT NULL,
                   detail BLOB
                 );
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        if version < 2 {
            // Encrypted file contents live apart from items so lists never load them.
            self.conn.execute_batch(
                "BEGIN;
                 CREATE TABLE attachments (
                   id       TEXT PRIMARY KEY,
                   item_id  TEXT NOT NULL,
                   vault_id TEXT NOT NULL,
                   blob     BLOB NOT NULL,
                   added_at INTEGER NOT NULL
                 );
                 CREATE INDEX attachments_item ON attachments(item_id);
                 PRAGMA user_version = 2;
                 COMMIT;",
            )?;
        }
        if version < 3 {
            // Highest version the server has shown us per object: anything older is a
            // rollback and gets rejected (security review M2).
            self.conn.execute_batch(
                "BEGIN;
                 CREATE TABLE sync_hwm (
                   entity  TEXT NOT NULL,
                   id      TEXT NOT NULL,
                   version INTEGER NOT NULL,
                   PRIMARY KEY (entity, id)
                 );
                 PRAGMA user_version = 3;
                 COMMIT;",
            )?;
        }
        if version < 4 {
            // Attachment blobs sync separately from items (large, fetched on demand).
            self.conn.execute_batch(
                "BEGIN;
                 ALTER TABLE attachments ADD COLUMN uploaded INTEGER NOT NULL DEFAULT 0;
                 CREATE TABLE attachment_deletes (id TEXT PRIMARY KEY);
                 PRAGMA user_version = 4;
                 COMMIT;",
            )?;
        }
        Ok(())
    }

    pub fn transaction<T>(&mut self, f: impl FnOnce(&Store) -> Result<T>) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        match f(self) {
            Ok(v) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    // ---- meta -------------------------------------------------------------------------

    pub fn meta_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn meta_set(&self, key: &str, value: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn meta_delete(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM meta WHERE key = ?1", [key])?;
        Ok(())
    }

    // ---- vaults -----------------------------------------------------------------------

    pub fn vaults(&self) -> Result<Vec<VaultRow>> {
        let mut st = self.conn.prepare(
            "SELECT id, wrapped_key, attrs, created_at, updated_at, revision FROM vaults WHERE deleted = 0 ORDER BY created_at",
        )?;
        let rows = st.query_map([], |r| {
            Ok(VaultRow {
                id: uuid_of(r.get(0)?)?,
                wrapped_key: r.get(1)?,
                attrs: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
                revision: r.get::<_, i64>(5)? as u64,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn put_vault(&self, v: &VaultRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO vaults(id, wrapped_key, attrs, created_at, updated_at, revision, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
             ON CONFLICT(id) DO UPDATE SET wrapped_key = excluded.wrapped_key, attrs = excluded.attrs,
               updated_at = excluded.updated_at, revision = excluded.revision, deleted = 0",
            params![v.id.to_string(), v.wrapped_key, v.attrs, v.created_at, v.updated_at, v.revision as i64],
        )?;
        self.enqueue(Entity::Vault, v.id, v.revision, Op::Upsert)
    }

    /// Marks a vault deleted, keeping an authenticated tombstone (in `attrs`) for sync.
    pub fn delete_vault(&self, id: Uuid, revision: u64, at: Timestamp, tombstone: &[u8]) -> Result<()> {
        self.conn.execute(
            "UPDATE vaults SET deleted = 1, wrapped_key = x'', attrs = ?4, updated_at = ?2, revision = ?3 WHERE id = ?1",
            params![id.to_string(), at, revision as i64, tombstone],
        )?;
        self.enqueue(Entity::Vault, id, revision, Op::Delete)
    }

    // ---- items ------------------------------------------------------------------------

    fn item_from(r: &rusqlite::Row) -> rusqlite::Result<ItemRow> {
        Ok(ItemRow {
            id: uuid_of(r.get(0)?)?,
            vault_id: uuid_of(r.get(1)?)?,
            revision: r.get::<_, i64>(2)? as u64,
            overview: r.get(3)?,
            details: r.get(4)?,
            created_at: r.get(5)?,
            updated_at: r.get(6)?,
        })
    }

    /// All live items (overview + details blobs).
    pub fn items(&self) -> Result<Vec<ItemRow>> {
        let mut st = self.conn.prepare(
            "SELECT id, vault_id, revision, overview, details, created_at, updated_at FROM items WHERE deleted = 0",
        )?;
        let rows = st.query_map([], Self::item_from)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn item(&self, id: Uuid) -> Result<Option<ItemRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, vault_id, revision, overview, details, created_at, updated_at FROM items WHERE id = ?1 AND deleted = 0",
                [id.to_string()],
                Self::item_from,
            )
            .optional()?)
    }

    pub fn put_item(&self, i: &ItemRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items(id, vault_id, revision, overview, details, created_at, updated_at, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
             ON CONFLICT(id) DO UPDATE SET vault_id = excluded.vault_id, revision = excluded.revision,
               overview = excluded.overview, details = excluded.details, updated_at = excluded.updated_at, deleted = 0",
            params![
                i.id.to_string(),
                i.vault_id.to_string(),
                i.revision as i64,
                i.overview,
                i.details,
                i.created_at,
                i.updated_at
            ],
        )?;
        self.enqueue(Entity::Item, i.id, i.revision, Op::Upsert)
    }

    /// Permanently removes an item's content, leaving an authenticated tombstone (stored
    /// in the overview column) for sync. The `deleted` column is only a cache of it.
    pub fn purge_item(&self, id: Uuid, revision: u64, at: Timestamp, tombstone: &[u8]) -> Result<()> {
        self.conn.execute(
            "UPDATE items SET deleted = 1, overview = ?4, details = NULL, updated_at = ?2, revision = ?3 WHERE id = ?1",
            params![id.to_string(), at, revision as i64, tombstone],
        )?;
        self.conn.execute("DELETE FROM item_history WHERE item_id = ?1", [id.to_string()])?;
        self.enqueue(Entity::Item, id, revision, Op::Delete)
    }

    // ---- history ----------------------------------------------------------------------

    pub fn push_history(&self, h: &HistoryRow, keep: usize) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO item_history(item_id, revision, vault_id, overview, details, saved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![h.item_id.to_string(), h.revision as i64, h.vault_id.to_string(), h.overview, h.details, h.saved_at],
        )?;
        self.conn.execute(
            "DELETE FROM item_history WHERE item_id = ?1 AND revision NOT IN
               (SELECT revision FROM item_history WHERE item_id = ?1 ORDER BY revision DESC LIMIT ?2)",
            params![h.item_id.to_string(), keep as i64],
        )?;
        Ok(())
    }

    pub fn history(&self, item_id: Uuid) -> Result<Vec<HistoryRow>> {
        let mut st = self.conn.prepare(
            "SELECT item_id, vault_id, revision, overview, details, saved_at FROM item_history
             WHERE item_id = ?1 ORDER BY revision DESC",
        )?;
        let rows = st.query_map([item_id.to_string()], |r| {
            Ok(HistoryRow {
                item_id: uuid_of(r.get(0)?)?,
                vault_id: uuid_of(r.get(1)?)?,
                revision: r.get::<_, i64>(2)? as u64,
                overview: r.get(3)?,
                details: r.get(4)?,
                saved_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn replace_history(&self, item_id: Uuid, rows: &[HistoryRow]) -> Result<()> {
        self.conn.execute("DELETE FROM item_history WHERE item_id = ?1", [item_id.to_string()])?;
        for h in rows {
            self.conn.execute(
                "INSERT INTO item_history(item_id, revision, vault_id, overview, details, saved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![h.item_id.to_string(), h.revision as i64, h.vault_id.to_string(), h.overview, h.details, h.saved_at],
            )?;
        }
        Ok(())
    }

    // ---- sync support -------------------------------------------------------------------

    /// Raw vault row including deleted ones (tombstone in `attrs`).
    pub fn vault_raw(&self, id: Uuid) -> Result<Option<(VaultRow, bool)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, wrapped_key, attrs, created_at, updated_at, revision, deleted FROM vaults WHERE id = ?1",
                [id.to_string()],
                |r| {
                    Ok((
                        VaultRow {
                            id: uuid_of(r.get(0)?)?,
                            wrapped_key: r.get(1)?,
                            attrs: r.get(2)?,
                            created_at: r.get(3)?,
                            updated_at: r.get(4)?,
                            revision: r.get::<_, i64>(5)? as u64,
                        },
                        r.get::<_, i64>(6)? != 0,
                    ))
                },
            )
            .optional()?)
    }

    /// Applies a vault row received from the server (no outbox entry).
    pub fn put_vault_remote(&self, v: &VaultRow, deleted: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO vaults(id, wrapped_key, attrs, created_at, updated_at, revision, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET wrapped_key = excluded.wrapped_key, attrs = excluded.attrs,
               updated_at = excluded.updated_at, revision = excluded.revision, deleted = excluded.deleted",
            params![v.id.to_string(), v.wrapped_key, v.attrs, v.created_at, v.updated_at, v.revision as i64, deleted as i64],
        )?;
        Ok(())
    }

    /// Raw item row including tombstones (tombstone envelope in `overview`, empty details).
    pub fn item_raw(&self, id: Uuid) -> Result<Option<(ItemRow, bool)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, vault_id, revision, overview, COALESCE(details, x''), created_at, updated_at, deleted FROM items WHERE id = ?1",
                [id.to_string()],
                |r| Ok((Self::item_from(r)?, r.get::<_, i64>(7)? != 0)),
            )
            .optional()?)
    }

    /// Applies an item row received from the server (no outbox entry).
    pub fn put_item_remote(&self, i: &ItemRow, deleted: bool) -> Result<()> {
        let details: Option<&[u8]> = if deleted { None } else { Some(&i.details) };
        self.conn.execute(
            "INSERT INTO items(id, vault_id, revision, overview, details, created_at, updated_at, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET vault_id = excluded.vault_id, revision = excluded.revision,
               overview = excluded.overview, details = excluded.details, updated_at = excluded.updated_at,
               deleted = excluded.deleted",
            params![i.id.to_string(), i.vault_id.to_string(), i.revision as i64, i.overview, details, i.created_at, i.updated_at, deleted as i64],
        )?;
        if deleted {
            self.conn.execute("DELETE FROM item_history WHERE item_id = ?1", [i.id.to_string()])?;
            self.conn.execute("DELETE FROM attachments WHERE item_id = ?1", [i.id.to_string()])?;
        }
        Ok(())
    }

    pub fn hwm(&self, entity: &str, id: Uuid) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT version FROM sync_hwm WHERE entity = ?1 AND id = ?2", params![entity, id.to_string()], |r| r.get::<_, i64>(0))
            .optional()?
            .unwrap_or(0) as u64)
    }

    pub fn set_hwm(&self, entity: &str, id: Uuid, version: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_hwm(entity, id, version) VALUES (?1, ?2, ?3)
             ON CONFLICT(entity, id) DO UPDATE SET version = MAX(version, excluded.version)",
            params![entity, id.to_string(), version as i64],
        )?;
        Ok(())
    }

    /// Distinct (entity, id) with local changes waiting to be pushed, up to `seq`.
    pub fn outbox_pending(&self) -> Result<(i64, Vec<(String, Uuid)>)> {
        let max: i64 = self.conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM outbox", [], |r| r.get(0))?;
        let mut st = self.conn.prepare("SELECT DISTINCT entity, entity_id FROM outbox WHERE seq <= ?1")?;
        let rows = st.query_map([max], |r| Ok((r.get::<_, String>(0)?, uuid_of(r.get(1)?)?)))?;
        Ok((max, rows.collect::<rusqlite::Result<_>>()?))
    }

    pub fn outbox_has(&self, entity: &str, id: Uuid) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM outbox WHERE entity = ?1 AND entity_id = ?2)",
            params![entity, id.to_string()],
            |r| r.get::<_, i64>(0),
        )? != 0)
    }

    pub fn outbox_clear(&self, upto: i64, entity: &str, id: Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM outbox WHERE seq <= ?1 AND entity = ?2 AND entity_id = ?3", params![upto, entity, id.to_string()])?;
        Ok(())
    }

    // ---- attachments ------------------------------------------------------------------

    pub fn put_attachment(&self, id: Uuid, item_id: Uuid, vault_id: Uuid, blob: &[u8], at: Timestamp) -> Result<()> {
        self.put_attachment_state(id, item_id, vault_id, blob, at, false)
    }

    /// `uploaded` = the server already has exactly this blob (downloaded from it).
    pub fn put_attachment_state(&self, id: Uuid, item_id: Uuid, vault_id: Uuid, blob: &[u8], at: Timestamp, uploaded: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO attachments(id, item_id, vault_id, blob, added_at, uploaded) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id.to_string(), item_id.to_string(), vault_id.to_string(), blob, at, uploaded as i64],
        )?;
        Ok(())
    }

    pub fn has_attachment(&self, id: Uuid) -> Result<bool> {
        Ok(self.conn.query_row("SELECT EXISTS(SELECT 1 FROM attachments WHERE id = ?1)", [id.to_string()], |r| r.get::<_, i64>(0))? != 0)
    }

    /// Blobs not yet on the server: (id, item_id, blob).
    pub fn attachments_to_upload(&self) -> Result<Vec<(Uuid, Uuid, Vec<u8>)>> {
        let mut st = self.conn.prepare("SELECT id, item_id, blob FROM attachments WHERE uploaded = 0 LIMIT 20")?;
        let rows = st.query_map([], |r| Ok((uuid_of(r.get(0)?)?, uuid_of(r.get(1)?)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn mark_attachment_uploaded(&self, id: Uuid) -> Result<()> {
        self.conn.execute("UPDATE attachments SET uploaded = 1 WHERE id = ?1", [id.to_string()])?;
        Ok(())
    }

    pub fn attachment_deletes(&self) -> Result<Vec<Uuid>> {
        let mut st = self.conn.prepare("SELECT id FROM attachment_deletes LIMIT 100")?;
        let rows = st.query_map([], |r| uuid_of(r.get(0)?))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn clear_attachment_delete(&self, id: Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM attachment_deletes WHERE id = ?1", [id.to_string()])?;
        Ok(())
    }

    pub fn attachment(&self, id: Uuid) -> Result<Option<(Uuid, Uuid, Vec<u8>)>> {
        Ok(self
            .conn
            .query_row("SELECT item_id, vault_id, blob FROM attachments WHERE id = ?1", [id.to_string()], |r| {
                Ok((uuid_of(r.get(0)?)?, uuid_of(r.get(1)?)?, r.get(2)?))
            })
            .optional()?)
    }

    pub fn attachments_of(&self, item_id: Uuid) -> Result<Vec<(Uuid, Uuid, Vec<u8>)>> {
        let mut st = self.conn.prepare("SELECT id, vault_id, blob FROM attachments WHERE item_id = ?1")?;
        let rows = st.query_map([item_id.to_string()], |r| Ok((uuid_of(r.get(0)?)?, uuid_of(r.get(1)?)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Removes a blob locally and queues its removal from the server.
    pub fn delete_attachment(&self, id: Uuid) -> Result<()> {
        self.conn.execute("DELETE FROM attachments WHERE id = ?1", [id.to_string()])?;
        self.conn.execute("INSERT OR IGNORE INTO attachment_deletes(id) VALUES (?1)", [id.to_string()])?;
        Ok(())
    }

    pub fn delete_attachments_of(&self, item_id: Uuid) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO attachment_deletes(id) SELECT id FROM attachments WHERE item_id = ?1",
            [item_id.to_string()],
        )?;
        self.conn.execute("DELETE FROM attachments WHERE item_id = ?1", [item_id.to_string()])?;
        Ok(())
    }

    // ---- outbox (sync foundation) -----------------------------------------------------

    fn enqueue(&self, entity: Entity, id: Uuid, revision: u64, op: Op) -> Result<()> {
        let entity = match entity {
            Entity::Vault => "vault",
            Entity::Item => "item",
        };
        let op = match op {
            Op::Upsert => "upsert",
            Op::Delete => "delete",
        };
        self.conn.execute(
            "INSERT INTO outbox(entity, entity_id, revision, op, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![entity, id.to_string(), revision as i64, op, crate::model::now_ms()],
        )?;
        Ok(())
    }

    pub fn outbox_len(&self) -> Result<u64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get::<_, i64>(0))? as u64)
    }

    // ---- events (security log) --------------------------------------------------------

    pub fn log_event(&self, kind: &str, detail: Option<&[u8]>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events(at, kind, detail) VALUES (?1, ?2, ?3)",
            params![crate::model::now_ms(), kind, detail],
        )?;
        // Keep the log bounded.
        self.conn.execute(
            "DELETE FROM events WHERE id NOT IN (SELECT id FROM events ORDER BY id DESC LIMIT 2000)",
            [],
        )?;
        Ok(())
    }

    pub fn events(&self, limit: usize) -> Result<Vec<(i64, Timestamp, String, Option<Vec<u8>>)>> {
        let mut st = self.conn.prepare("SELECT id, at, kind, detail FROM events ORDER BY id DESC LIMIT ?1")?;
        let rows = st.query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Consistent online backup of the database into `dest`.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        let dest = dest.to_string_lossy().replace('\'', "''");
        self.conn.execute_batch(&format!("VACUUM INTO '{dest}'"))?;
        Ok(())
    }
}
