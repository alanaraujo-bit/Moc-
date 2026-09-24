//! Sessions: random 256-bit bearer tokens handed out after a successful signed login,
//! stored only as SHA-256 hashes, bound to a device, revocable per device. A token lets a
//! device fetch ciphertext — it can never decrypt anything.

use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use data_encoding::BASE64URL_NOPAD;
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::ApiError;
use crate::Shared;

pub const SESSION_DAYS: i64 = 90;

pub struct Session {
    pub account_id: Uuid,
    pub device_id: Uuid,
    pub token_hash: Vec<u8>,
}

pub fn new_token() -> (String, Vec<u8>) {
    let mut raw = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut raw);
    let token = BASE64URL_NOPAD.encode(&raw);
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    (token, hash)
}

impl FromRequestParts<Shared> for Session {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &Shared) -> Result<Self, Self::Rejection> {
        let header = parts.headers.get("authorization").and_then(|v| v.to_str().ok()).ok_or_else(ApiError::unauthorized)?;
        let token = header.strip_prefix("Bearer ").ok_or_else(ApiError::unauthorized)?;
        let hash = Sha256::digest(token.trim().as_bytes()).to_vec();
        let row: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT s.account_id, s.device_id FROM sessions s JOIN devices d ON d.id = s.device_id
             WHERE s.token_hash = $1 AND s.expires_at > now() AND d.revoked_at IS NULL",
        )
        .bind(&hash)
        .fetch_optional(&state.db)
        .await?;
        let (account_id, device_id) = row.ok_or_else(ApiError::unauthorized)?;
        // Best effort "last seen" for the devices list.
        let _ = sqlx::query("UPDATE devices SET last_seen_at = now() WHERE id = $1 AND last_seen_at < now() - interval '5 minutes'")
            .bind(device_id)
            .execute(&state.db)
            .await;
        Ok(Session { account_id, device_id, token_hash: hash })
    }
}

/// Client IP as seen by Railway's proxy (first hop in X-Forwarded-For).
pub fn client_ip(parts: &axum::http::HeaderMap) -> String {
    parts
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "local".into())
}

#[allow(dead_code)]
pub type AppState = State<Shared>;
