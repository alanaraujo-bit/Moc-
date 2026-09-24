//! Encrypted attachment blobs. The server stores and returns opaque bytes; per-account
//! quota keeps storage predictable.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::Session;
use crate::error::{ApiError, ApiResult};
use crate::Shared;

pub const MAX_BLOB: usize = 26 * 1024 * 1024;
const QUOTA_BYTES: i64 = 1024 * 1024 * 1024; // 1 GB per account for now

#[derive(Deserialize)]
pub struct PutQuery {
    item: Uuid,
}

pub async fn put(State(state): State<Shared>, session: Session, Path(id): Path<Uuid>, Query(q): Query<PutQuery>, body: Bytes) -> ApiResult<StatusCode> {
    store(&state, session.account_id, id, q.item, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores a blob in `account` (counted against that account's quota).
pub(crate) async fn store(state: &Shared, account: Uuid, id: Uuid, item: Uuid, body: &[u8]) -> ApiResult<()> {
    if body.len() > MAX_BLOB || body.is_empty() {
        return Err(ApiError::bad_request("Arquivo grande demais."));
    }
    let (used,): (Option<i64>,) = sqlx::query_as("SELECT sum(size)::bigint FROM attachments WHERE account_id = $1 AND id <> $2")
        .bind(account)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    if used.unwrap_or(0) + body.len() as i64 > QUOTA_BYTES {
        return Err(ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "quota", "O espaço para anexos da sua conta acabou."));
    }
    sqlx::query(
        "INSERT INTO attachments(account_id, id, item_id, blob, size) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (account_id, id) DO UPDATE SET item_id = $3, blob = $4, size = $5",
    )
    .bind(account)
    .bind(id)
    .bind(item)
    .bind(body)
    .bind(body.len() as i64)
    .execute(&state.db)
    .await?;
    Ok(())
}

pub async fn get(State(state): State<Shared>, session: Session, Path(id): Path<Uuid>) -> ApiResult<impl IntoResponse> {
    let row: Option<(Uuid, Vec<u8>)> = sqlx::query_as("SELECT item_id, blob FROM attachments WHERE account_id = $1 AND id = $2")
        .bind(session.account_id)
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    let (item, blob) = row.ok_or_else(ApiError::not_found)?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream".to_string()), (header::HeaderName::from_static("x-item-id"), item.to_string())], blob))
}

pub async fn delete(State(state): State<Shared>, session: Session, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM attachments WHERE account_id = $1 AND id = $2").bind(session.account_id).bind(id).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
