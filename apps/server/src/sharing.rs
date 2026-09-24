//! Shared vaults. Same rules as `moco_core::account::sharing::fake::FakeCloud`.
//!
//! The vault and its items stay in the owner's account. A member row holds the owner's
//! signed grant (vault key sealed to the member); the server checks the signature so the
//! role it enforces is the one the owner signed, but it can't open the key. Members read
//! and write the owner's rows by membership: readers can't push, writes must stay in the
//! vault and be sealed with the current key generation (after a removal the owner rotates
//! the key and re-grants, so stale-key writes are refused).

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use moco_core::account::sharing::{Member, SharedEntry};
use moco_core::account::sync::{PullResponse, PushRequest, PushResponse};
use moco_core::crypto::identity::PublicIdentity;
use moco_core::crypto::share::{self, Role, ShareGrant};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;

use crate::account::normalize_email;
use crate::auth::{client_ip, Session};
use crate::error::{ApiError, ApiResult};
use crate::sync::{self, ItemTuple, VaultTuple, MAX_ROWS_PER_PULL, MAX_ROWS_PER_PUSH};
use crate::{files, Shared};

const MAX_MEMBERS: i64 = 100;

fn identity_of(record: &str) -> ApiResult<PublicIdentity> {
    let rec: Value = serde_json::from_str(record).map_err(|_| ApiError::bad_request("Registro inválido."))?;
    serde_json::from_value(rec["publicIdentity"].clone()).map_err(|_| ApiError::bad_request("Registro sem chave pública."))
}

fn role_of(s: &str) -> ApiResult<Role> {
    serde_json::from_value(Value::String(s.into())).map_err(|_| ApiError::bad_request("Papel inválido."))
}

fn role_str(r: Role) -> &'static str {
    match r {
        Role::Owner => "owner",
        Role::Editor => "editor",
        Role::Reader => "reader",
    }
}

fn not_member() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_member", "Este cofre não está mais compartilhado com você.")
}

/// Key generation a vault's writes must use: the newest one the owner granted.
async fn required_gen(state: &Shared, owner: Uuid, vault: Uuid) -> ApiResult<u32> {
    let (gen,): (Option<i32>,) = sqlx::query_as("SELECT max(key_gen) FROM vault_members WHERE owner_id = $1 AND vault_id = $2")
        .bind(owner)
        .bind(vault)
        .fetch_one(&state.db)
        .await?;
    Ok(gen.unwrap_or(0).max(0) as u32)
}

async fn membership(state: &Shared, member: Uuid, owner: Uuid, vault: Uuid) -> ApiResult<Role> {
    let row: Option<(String,)> = sqlx::query_as("SELECT role FROM vault_members WHERE owner_id = $1 AND vault_id = $2 AND member_id = $3")
        .bind(owner)
        .bind(vault)
        .bind(member)
        .fetch_optional(&state.db)
        .await?;
    role_of(&row.ok_or_else(not_member)?.0)
}

async fn writer(state: &Shared, member: Uuid, owner: Uuid, vault: Uuid) -> ApiResult<()> {
    if !membership(state, member, owner, vault).await?.can_write() {
        return Err(ApiError::forbidden("Você só pode ver este cofre."));
    }
    Ok(())
}

// ---- people -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PeopleQuery {
    email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    account_id: Uuid,
    email: String,
    identity: PublicIdentity,
}

/// Finds someone to share with. This does reveal whether an email has an account (sharing
/// needs it), so it is signed-in only and rate limited per account and per IP.
pub async fn people(State(state): State<Shared>, session: Session, headers: HeaderMap, Query(q): Query<PeopleQuery>) -> ApiResult<Json<Person>> {
    if !state.limits.check(&format!("people:{}", session.account_id), 30, Duration::from_secs(3600))
        || !state.limits.check(&format!("people-ip:{}", client_ip(&headers)), 60, Duration::from_secs(3600))
    {
        return Err(ApiError::too_many());
    }
    let email = normalize_email(&q.email)?;
    let row: Option<(Uuid, String)> = sqlx::query_as("SELECT id, record FROM accounts WHERE email = $1").bind(&email).fetch_optional(&state.db).await?;
    let (account_id, record) = row.ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "no_account", "Ninguém usa o Mocó com esse e-mail."))?;
    Ok(Json(Person { account_id, email, identity: identity_of(&record)? }))
}

// ---- owner: members -----------------------------------------------------------------------

pub async fn put_member(
    State(state): State<Shared>,
    session: Session,
    Path((vault, member)): Path<(Uuid, Uuid)>,
    Json(g): Json<ShareGrant>,
) -> ApiResult<StatusCode> {
    let owner = session.account_id;
    if g.owner_id != owner || g.vault_id != vault || g.member_id != member || member == owner || g.role == Role::Owner {
        return Err(ApiError::bad_request("Convite inválido."));
    }
    let vrow: Option<(bool,)> = sqlx::query_as("SELECT deleted FROM vaults WHERE account_id = $1 AND id = $2").bind(owner).bind(vault).fetch_optional(&state.db).await?;
    if vrow.is_none_or(|(deleted,)| deleted) {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "not_found", "Sincronize o cofre antes de compartilhá-lo."));
    }
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM accounts WHERE id = $1").bind(member).fetch_optional(&state.db).await?;
    if exists.is_none() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "no_account", "Essa pessoa não tem mais conta no Mocó."));
    }
    // The role we enforce must be the one the owner signed.
    let (record,): (String,) = sqlx::query_as("SELECT record FROM accounts WHERE id = $1").bind(owner).fetch_one(&state.db).await?;
    share::verify(&g, &identity_of(&record)?.ed25519).map_err(|_| ApiError::bad_request("Assinatura do convite inválida."))?;

    let mut tx = state.db.begin().await?;
    // Serialize membership changes per vault owner.
    sqlx::query("SELECT id FROM accounts WHERE id = $1 FOR UPDATE").bind(owner).execute(&mut *tx).await?;
    let (current, count): (Option<i32>, i64) =
        sqlx::query_as("SELECT max(key_gen), count(*) FILTER (WHERE member_id <> $3) FROM vault_members WHERE owner_id = $1 AND vault_id = $2")
            .bind(owner)
            .bind(vault)
            .bind(member)
            .fetch_one(&mut *tx)
            .await?;
    if (g.key_gen as i64) < current.unwrap_or(0) as i64 {
        return Err(ApiError::conflict("A chave deste cofre mudou. Tente de novo."));
    }
    if count >= MAX_MEMBERS {
        return Err(ApiError::bad_request("Este cofre já tem o máximo de pessoas."));
    }
    let grant_json = serde_json::to_string(&g).map_err(|_| ApiError::bad_request("Convite inválido."))?;
    sqlx::query(
        "INSERT INTO vault_members(owner_id, vault_id, member_id, role, key_gen, grant_json) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (owner_id, vault_id, member_id) DO UPDATE SET role = $4, key_gen = $5, grant_json = $6, updated_at = now()",
    )
    .bind(owner)
    .bind(vault)
    .bind(member)
    .bind(role_str(g.role))
    .bind(g.key_gen as i32)
    .bind(&grant_json)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_members(State(state): State<Shared>, session: Session, Path(vault): Path<Uuid>) -> ApiResult<Json<Vec<Member>>> {
    let rows: Vec<(Uuid, String, String, String, i32)> = sqlx::query_as(
        "SELECT m.member_id, a.email, m.role, a.record, m.key_gen FROM vault_members m JOIN accounts a ON a.id = m.member_id
         WHERE m.owner_id = $1 AND m.vault_id = $2 ORDER BY m.created_at",
    )
    .bind(session.account_id)
    .bind(vault)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (account_id, email, role, record, key_gen) in rows {
        out.push(Member { account_id, email, role: role_of(&role)?, identity: identity_of(&record)?, key_gen: key_gen.max(0) as u32 });
    }
    Ok(Json(out))
}

/// The owner removes someone, or a member leaves.
pub async fn delete_member(State(state): State<Shared>, session: Session, Path((vault, member)): Path<(Uuid, Uuid)>) -> ApiResult<StatusCode> {
    let me = session.account_id;
    sqlx::query("DELETE FROM vault_members WHERE vault_id = $1 AND member_id = $2 AND (owner_id = $3 OR member_id = $3)")
        .bind(vault)
        .bind(member)
        .bind(me)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- member: shared vaults ------------------------------------------------------------------

pub async fn shared(State(state): State<Shared>, session: Session) -> ApiResult<Json<Vec<SharedEntry>>> {
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT m.owner_id, a.email, a.record, m.grant_json FROM vault_members m JOIN accounts a ON a.id = m.owner_id
         WHERE m.member_id = $1 ORDER BY m.created_at",
    )
    .bind(session.account_id)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (owner_id, owner_email, record, grant) in rows {
        let grant: ShareGrant = serde_json::from_str(&grant).map_err(|_| ApiError::bad_request("Convite inválido."))?;
        out.push(SharedEntry { owner_id, owner_email, owner_identity: identity_of(&record)?, grant });
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct SharedPullQuery {
    since: i64,
}

/// The vault row (always — the client needs it to check the grant) plus its items changed
/// after `since`. Never the owner's account record.
pub async fn shared_pull(
    State(state): State<Shared>,
    session: Session,
    Path((owner, vault)): Path<(Uuid, Uuid)>,
    Query(q): Query<SharedPullQuery>,
) -> ApiResult<Json<PullResponse>> {
    membership(&state, session.account_id, owner, vault).await?;
    // Cursor first: everything at or below it is already committed (seq is assigned under
    // the account lock), so nothing written meanwhile can fall behind the cursor.
    let (owner_seq,): (i64,) = sqlx::query_as("SELECT seq FROM accounts WHERE id = $1").bind(owner).fetch_one(&state.db).await?;
    let vault_row: Option<VaultTuple> = sqlx::query_as(
        "SELECT id, version, wrapped_key, attrs, deleted, created_at, updated_at, seq FROM vaults WHERE account_id = $1 AND id = $2",
    )
    .bind(owner)
    .bind(vault)
    .fetch_optional(&state.db)
    .await?;
    let items: Vec<ItemTuple> = sqlx::query_as(
        "SELECT id, vault_id, version, overview, details, deleted, created_at, updated_at, seq FROM items
         WHERE account_id = $1 AND vault_id = $2 AND seq > $3 AND seq <= $4 ORDER BY seq LIMIT $5",
    )
    .bind(owner)
    .bind(vault)
    .bind(q.since)
    .bind(owner_seq)
    .bind(MAX_ROWS_PER_PULL)
    .fetch_all(&state.db)
    .await?;
    let truncated = items.len() as i64 == MAX_ROWS_PER_PULL;
    let items: Vec<_> = items.into_iter().map(sync::item_of).collect();
    let cursor = if truncated { items.last().map(|(_, s)| *s).unwrap_or(q.since) } else { owner_seq };
    Ok(Json(PullResponse {
        cursor,
        vaults: vault_row.map(sync::vault_of).map(|(v, _)| v).into_iter().collect(),
        items: items.into_iter().map(|(i, _)| i).collect(),
        record: None,
        record_version: 0,
    }))
}

/// Envelope header: key id at bytes 52..68, key generation at 68..72 (see `crypto::aead`).
fn sealed_for(envelope: &[u8], vault: Uuid, min_gen: u32) -> bool {
    if envelope.len() < 72 {
        return false;
    }
    let key_id = Uuid::from_slice(&envelope[52..68]).unwrap_or_default();
    let gen = u32::from_be_bytes(envelope[68..72].try_into().expect("4 bytes"));
    key_id == vault && gen >= min_gen
}

pub async fn shared_push(
    State(state): State<Shared>,
    session: Session,
    Path((owner, vault)): Path<(Uuid, Uuid)>,
    Json(req): Json<PushRequest>,
) -> ApiResult<Json<PushResponse>> {
    if req.items.len() > MAX_ROWS_PER_PUSH {
        return Err(ApiError::bad_request("Muitas alterações de uma vez."));
    }
    if !req.vaults.is_empty() {
        return Err(ApiError::forbidden("Só quem criou o cofre pode alterá-lo."));
    }
    writer(&state, session.account_id, owner, vault).await?;
    let min_gen = required_gen(&state, owner, vault).await?;
    for p in &req.items {
        let r = &p.row;
        if r.vault_id != vault || !sealed_for(&r.overview, vault, min_gen) || (!r.details.is_empty() && !sealed_for(&r.details, vault, min_gen)) {
            return Err(ApiError::conflict("A chave deste cofre mudou. Sincronize de novo."));
        }
    }
    let ids: Vec<Uuid> = req.items.iter().map(|p| p.row.id).collect();
    let mut tx = state.db.begin().await?;
    // Take the owner's write lock before checking, so the check and the write agree.
    sqlx::query("SELECT seq FROM accounts WHERE id = $1 FOR UPDATE").bind(owner).execute(&mut *tx).await?;
    let (elsewhere,): (i64,) = sqlx::query_as("SELECT count(*) FROM items WHERE account_id = $1 AND id = ANY($2) AND vault_id <> $3")
        .bind(owner)
        .bind(&ids)
        .bind(vault)
        .fetch_one(&mut *tx)
        .await?;
    if elsewhere > 0 {
        return Err(ApiError::forbidden("Esse item não pertence a este cofre."));
    }
    let resp = sync::apply_push(&mut tx, owner, &req).await?;
    tx.commit().await?;
    Ok(Json(resp))
}

// ---- member: attachments --------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AttachmentQuery {
    item: Uuid,
}

/// Item id of an attachment in the owner's account, if it belongs to an item of this vault.
async fn attachment_in_vault(state: &Shared, owner: Uuid, vault: Uuid, id: Uuid) -> ApiResult<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT a.item_id FROM attachments a JOIN items i ON i.account_id = a.account_id AND i.id = a.item_id
         WHERE a.account_id = $1 AND a.id = $2 AND i.vault_id = $3",
    )
    .bind(owner)
    .bind(id)
    .bind(vault)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(i,)| i))
}

pub async fn shared_attachment_get(State(state): State<Shared>, session: Session, Path((owner, vault, id)): Path<(Uuid, Uuid, Uuid)>) -> ApiResult<impl IntoResponse> {
    membership(&state, session.account_id, owner, vault).await?;
    let row: Option<(Uuid, Vec<u8>)> = sqlx::query_as(
        "SELECT a.item_id, a.blob FROM attachments a JOIN items i ON i.account_id = a.account_id AND i.id = a.item_id
         WHERE a.account_id = $1 AND a.id = $2 AND i.vault_id = $3",
    )
    .bind(owner)
    .bind(id)
    .bind(vault)
    .fetch_optional(&state.db)
    .await?;
    let (item, blob) = row.ok_or_else(ApiError::not_found)?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream".to_string()), (header::HeaderName::from_static("x-item-id"), item.to_string())], blob))
}

pub async fn shared_attachment_put(
    State(state): State<Shared>,
    session: Session,
    Path((owner, vault, id)): Path<(Uuid, Uuid, Uuid)>,
    Query(q): Query<AttachmentQuery>,
    body: Bytes,
) -> ApiResult<StatusCode> {
    writer(&state, session.account_id, owner, vault).await?;
    let item: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM items WHERE account_id = $1 AND id = $2 AND vault_id = $3 AND NOT deleted")
        .bind(owner)
        .bind(q.item)
        .bind(vault)
        .fetch_optional(&state.db)
        .await?;
    if item.is_none() {
        return Err(ApiError::not_found());
    }
    // An existing attachment with this id must already belong to the same item (no
    // overwriting the owner's files elsewhere by reusing an id).
    let existing: Option<(Uuid,)> =
        sqlx::query_as("SELECT item_id FROM attachments WHERE account_id = $1 AND id = $2").bind(owner).bind(id).fetch_optional(&state.db).await?;
    if existing.is_some_and(|(i,)| i != q.item) {
        return Err(ApiError::forbidden("Esse anexo não pertence a este item."));
    }
    files::store(&state, owner, id, q.item, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn shared_attachment_delete(State(state): State<Shared>, session: Session, Path((owner, vault, id)): Path<(Uuid, Uuid, Uuid)>) -> ApiResult<StatusCode> {
    writer(&state, session.account_id, owner, vault).await?;
    if attachment_in_vault(&state, owner, vault, id).await?.is_some() {
        sqlx::query("DELETE FROM attachments WHERE account_id = $1 AND id = $2").bind(owner).bind(id).execute(&state.db).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}
