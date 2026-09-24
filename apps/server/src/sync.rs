//! Incremental sync. Same rules as `moco_core::account::sync::fake::FakeServer`:
//! a push is accepted only when its base equals the stored version and it moves the
//! version forward; otherwise the current row comes back as a conflict. Every accepted
//! write gets the next per-account sequence number; pulls return rows above a cursor.

use axum::extract::{Query, State};
use axum::Json;
use moco_core::account::sync::{PullResponse, PushRequest, PushResponse, WireItem, WireVault};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::Session;
use crate::error::{ApiError, ApiResult};
use crate::Shared;

const MAX_ROWS_PER_PUSH: usize = 5_000;
const MAX_ROWS_PER_PULL: i64 = 2_000;

#[derive(Deserialize)]
pub struct PullQuery {
    since: i64,
    #[serde(default, rename = "recordVersion")]
    record_version: i64,
}

type VaultTuple = (Uuid, i64, Vec<u8>, Vec<u8>, bool, i64, i64, i64);
type ItemTuple = (Uuid, Uuid, i64, Vec<u8>, Vec<u8>, bool, i64, i64, i64);

fn vault_of(t: VaultTuple) -> (WireVault, i64) {
    (
        WireVault { id: t.0, version: t.1 as u64, wrapped_key: t.2, attrs: t.3, deleted: t.4, created_at: t.5, updated_at: t.6 },
        t.7,
    )
}

fn item_of(t: ItemTuple) -> (WireItem, i64) {
    (
        WireItem { id: t.0, vault_id: t.1, version: t.2 as u64, overview: t.3, details: t.4, deleted: t.5, created_at: t.6, updated_at: t.7 },
        t.8,
    )
}

pub async fn pull(State(state): State<Shared>, session: Session, Query(q): Query<PullQuery>) -> ApiResult<Json<PullResponse>> {
    let acct = session.account_id;
    let vaults: Vec<VaultTuple> = sqlx::query_as(
        "SELECT id, version, wrapped_key, attrs, deleted, created_at, updated_at, seq FROM vaults
         WHERE account_id = $1 AND seq > $2 ORDER BY seq",
    )
    .bind(acct)
    .bind(q.since)
    .fetch_all(&state.db)
    .await?;
    let items: Vec<ItemTuple> = sqlx::query_as(
        "SELECT id, vault_id, version, overview, details, deleted, created_at, updated_at, seq FROM items
         WHERE account_id = $1 AND seq > $2 ORDER BY seq LIMIT $3",
    )
    .bind(acct)
    .bind(q.since)
    .bind(MAX_ROWS_PER_PULL)
    .fetch_all(&state.db)
    .await?;
    let (acct_seq, record, record_version): (i64, String, i64) =
        sqlx::query_as("SELECT seq, record, record_version FROM accounts WHERE id = $1").bind(acct).fetch_one(&state.db).await?;

    // If items were truncated, the cursor stops at the last item returned so the client
    // pulls the rest next round (vaults are few and always complete).
    let truncated = items.len() as i64 == MAX_ROWS_PER_PULL;
    let items: Vec<(WireItem, i64)> = items.into_iter().map(item_of).collect();
    let cursor = if truncated { items.last().map(|(_, s)| *s).unwrap_or(q.since) } else { acct_seq };
    Ok(Json(PullResponse {
        cursor,
        vaults: vaults.into_iter().map(vault_of).map(|(v, _)| v).collect(),
        items: items.into_iter().map(|(i, _)| i).collect(),
        record: (record_version > q.record_version).then_some(record),
        record_version: record_version as u64,
    }))
}

pub async fn push(State(state): State<Shared>, session: Session, Json(req): Json<PushRequest>) -> ApiResult<Json<PushResponse>> {
    if req.vaults.len() + req.items.len() > MAX_ROWS_PER_PUSH {
        return Err(ApiError::bad_request("Muitas alterações de uma vez."));
    }
    let acct = session.account_id;
    let mut tx = state.db.begin().await?;
    // Serialize writers per account (sequence numbers must be gap-free and ordered).
    let (mut seq,): (i64,) = sqlx::query_as("SELECT seq FROM accounts WHERE id = $1 FOR UPDATE").bind(acct).fetch_one(&mut *tx).await?;
    let mut resp = PushResponse::default();

    for p in &req.vaults {
        let r = &p.row;
        let current: Option<VaultTuple> = sqlx::query_as(
            "SELECT id, version, wrapped_key, attrs, deleted, created_at, updated_at, seq FROM vaults WHERE account_id = $1 AND id = $2",
        )
        .bind(acct)
        .bind(r.id)
        .fetch_optional(&mut *tx)
        .await?;
        let cur_version = current.as_ref().map(|c| c.1 as u64).unwrap_or(0);
        if p.base == cur_version && r.version > cur_version {
            seq += 1;
            sqlx::query(
                "INSERT INTO vaults(account_id, id, version, wrapped_key, attrs, deleted, created_at, updated_at, seq)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT (account_id, id) DO UPDATE SET version = $3, wrapped_key = $4, attrs = $5, deleted = $6, updated_at = $8, seq = $9",
            )
            .bind(acct)
            .bind(r.id)
            .bind(r.version as i64)
            .bind(&r.wrapped_key)
            .bind(&r.attrs)
            .bind(r.deleted)
            .bind(r.created_at)
            .bind(r.updated_at)
            .bind(seq)
            .execute(&mut *tx)
            .await?;
            resp.accepted.push(("vault".into(), r.id, r.version));
        } else if let Some(c) = current {
            resp.conflict_vaults.push(vault_of(c).0);
        }
    }

    for p in &req.items {
        let r = &p.row;
        let current: Option<ItemTuple> = sqlx::query_as(
            "SELECT id, vault_id, version, overview, details, deleted, created_at, updated_at, seq FROM items WHERE account_id = $1 AND id = $2",
        )
        .bind(acct)
        .bind(r.id)
        .fetch_optional(&mut *tx)
        .await?;
        let cur_version = current.as_ref().map(|c| c.2 as u64).unwrap_or(0);
        if p.base == cur_version && r.version > cur_version {
            seq += 1;
            sqlx::query(
                "INSERT INTO items(account_id, id, vault_id, version, overview, details, deleted, created_at, updated_at, seq)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                 ON CONFLICT (account_id, id) DO UPDATE SET vault_id = $3, version = $4, overview = $5, details = $6,
                   deleted = $7, updated_at = $9, seq = $10",
            )
            .bind(acct)
            .bind(r.id)
            .bind(r.vault_id)
            .bind(r.version as i64)
            .bind(&r.overview)
            .bind(&r.details)
            .bind(r.deleted)
            .bind(r.created_at)
            .bind(r.updated_at)
            .bind(seq)
            .execute(&mut *tx)
            .await?;
            if r.deleted {
                sqlx::query("DELETE FROM attachments WHERE account_id = $1 AND item_id = $2").bind(acct).bind(r.id).execute(&mut *tx).await?;
            }
            resp.accepted.push(("item".into(), r.id, r.version));
        } else if let Some(c) = current {
            resp.conflict_items.push(item_of(c).0);
        }
    }

    sqlx::query("UPDATE accounts SET seq = $2 WHERE id = $1").bind(acct).bind(seq).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(resp))
}
