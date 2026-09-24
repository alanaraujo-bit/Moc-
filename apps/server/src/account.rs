//! Accounts, login, devices, two-step verification.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use data_encoding::BASE64;
use ed25519_dalek::{Signature, VerifyingKey};
use moco_core::totp::TotpConfig;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;

use crate::auth::{client_ip, new_token, Session, SESSION_DAYS};
use crate::error::{ApiError, ApiResult};
use crate::Shared;

const MINUTE: Duration = Duration::from_secs(60);

fn normalize_email(e: &str) -> ApiResult<String> {
    let e = e.trim().to_lowercase();
    if e.len() < 5 || e.len() > 200 || !e.contains('@') || !e.split('@').nth(1).is_some_and(|d| d.contains('.')) {
        return Err(ApiError::bad_request("Confira o e-mail."));
    }
    Ok(e)
}

fn b64(v: &str) -> ApiResult<Vec<u8>> {
    BASE64.decode(v.as_bytes()).map_err(|_| ApiError::bad_request("Codificação inválida."))
}

async fn audit(state: &Shared, account: Uuid, kind: &str, device: Option<Uuid>) {
    let _ = sqlx::query("INSERT INTO audit(account_id, kind, device_id) VALUES ($1, $2, $3)")
        .bind(account)
        .bind(kind)
        .bind(device)
        .execute(&state.db)
        .await;
}

// ---- pre-login ------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PreloginReq {
    email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreloginResp {
    account_id: Uuid,
    kdf: Value,
}

/// Returns the account id and KDF parameters for an email. Unknown emails get stable
/// decoy values, so this endpoint doesn't reveal who has an account.
pub async fn prelogin(State(state): State<Shared>, headers: HeaderMap, Json(req): Json<PreloginReq>) -> ApiResult<Json<PreloginResp>> {
    if !state.limits.check(&format!("pre:{}", client_ip(&headers)), 30, MINUTE) {
        return Err(ApiError::too_many());
    }
    let email = normalize_email(&req.email)?;
    let row: Option<(Uuid, String)> = sqlx::query_as("SELECT id, record FROM accounts WHERE email = $1").bind(&email).fetch_optional(&state.db).await?;
    if let Some((id, record)) = row {
        let rec: Value = serde_json::from_str(&record).map_err(|_| ApiError::bad_request("registro"))?;
        return Ok(Json(PreloginResp { account_id: id, kdf: rec["kdf"].clone() }));
    }
    let id_bytes = state.secrets.prf("decoy-id", email.as_bytes(), 16);
    let salt = state.secrets.prf("decoy-salt", email.as_bytes(), 16);
    Ok(Json(PreloginResp {
        account_id: Uuid::from_slice(&id_bytes).expect("16 bytes"),
        kdf: serde_json::json!({ "algorithm": "argon2id", "memoryKib": 65536, "iterations": 3, "parallelism": 4, "salt": BASE64.encode(&salt) }),
    }))
}

// ---- register ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    id: Uuid,
    name: String,
    platform: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterReq {
    email: String,
    account_id: Uuid,
    record: String,
    auth_public_key: String,
    device: DeviceInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResp {
    token: String,
    record: String,
    record_version: i64,
}

fn check_record(record: &str, account_id: Uuid, auth_public_key: &[u8]) -> ApiResult<()> {
    let rec: Value = serde_json::from_str(record).map_err(|_| ApiError::bad_request("Registro inválido."))?;
    if rec["id"].as_str() != Some(&account_id.to_string()) {
        return Err(ApiError::bad_request("Registro não confere com a conta."));
    }
    if rec["authPublicKey"].as_str().map(|s| BASE64.decode(s.as_bytes()).ok()) != Some(Some(auth_public_key.to_vec())) {
        return Err(ApiError::bad_request("Chave de acesso não confere com o registro."));
    }
    if record.len() > 64 * 1024 {
        return Err(ApiError::bad_request("Registro grande demais."));
    }
    Ok(())
}

async fn open_session(state: &Shared, account: Uuid, device: &DeviceInfo) -> ApiResult<String> {
    let name: String = device.name.chars().take(80).collect();
    let platform: String = device.platform.chars().take(30).collect();
    let existing: Option<(Uuid, bool)> =
        sqlx::query_as("SELECT account_id, revoked_at IS NOT NULL FROM devices WHERE id = $1").bind(device.id).fetch_optional(&state.db).await?;
    match existing {
        Some((owner, _)) if owner != account => return Err(ApiError::bad_request("Dispositivo inválido.")),
        Some((_, true)) => {
            return Err(ApiError::forbidden(
                "Este dispositivo foi removido da sua conta. Para usá-lo de novo, apague os dados do Mocó nele e entre como um dispositivo novo.",
            ))
        }
        Some(_) => {
            sqlx::query("UPDATE devices SET name = $2, platform = $3, last_seen_at = now() WHERE id = $1")
                .bind(device.id)
                .bind(&name)
                .bind(&platform)
                .execute(&state.db)
                .await?;
        }
        None => {
            sqlx::query("INSERT INTO devices(id, account_id, name, platform) VALUES ($1, $2, $3, $4)")
                .bind(device.id)
                .bind(account)
                .bind(&name)
                .bind(&platform)
                .execute(&state.db)
                .await?;
        }
    }
    let (token, hash) = new_token();
    sqlx::query("INSERT INTO sessions(token_hash, account_id, device_id, expires_at) VALUES ($1, $2, $3, now() + make_interval(days => $4))")
        .bind(&hash)
        .bind(account)
        .bind(device.id)
        .bind(SESSION_DAYS as i32)
        .execute(&state.db)
        .await?;
    Ok(token)
}

pub async fn register(State(state): State<Shared>, headers: HeaderMap, Json(req): Json<RegisterReq>) -> ApiResult<(StatusCode, Json<SessionResp>)> {
    if !state.limits.check(&format!("reg:{}", client_ip(&headers)), 5, Duration::from_secs(3600)) {
        return Err(ApiError::too_many());
    }
    let email = normalize_email(&req.email)?;
    let pk = b64(&req.auth_public_key)?;
    if pk.len() != 32 || VerifyingKey::from_bytes(pk.as_slice().try_into().expect("32")).is_err() {
        return Err(ApiError::bad_request("Chave de acesso inválida."));
    }
    check_record(&req.record, req.account_id, &pk)?;
    let inserted = sqlx::query("INSERT INTO accounts(id, email, record, auth_public_key) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING")
        .bind(req.account_id)
        .bind(&email)
        .bind(&req.record)
        .bind(&pk)
        .execute(&state.db)
        .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::conflict("Já existe uma conta com esse e-mail. Entre com ela."));
    }
    let token = open_session(&state, req.account_id, &req.device).await?;
    audit(&state, req.account_id, "account.created", Some(req.device.id)).await;
    Ok((StatusCode::CREATED, Json(SessionResp { token, record: req.record, record_version: 1 })))
}

// ---- login ------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeReq {
    account_id: Uuid,
}

#[derive(Serialize)]
pub struct ChallengeResp {
    nonce: String,
}

pub async fn challenge(State(state): State<Shared>, headers: HeaderMap, Json(req): Json<ChallengeReq>) -> ApiResult<Json<ChallengeResp>> {
    if !state.limits.check(&format!("chal:{}", client_ip(&headers)), 30, MINUTE) {
        return Err(ApiError::too_many());
    }
    let mut nonce = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    // Stored even for unknown accounts, so responses look identical.
    sqlx::query("INSERT INTO challenges(nonce, account_id, expires_at) VALUES ($1, $2, now() + interval '2 minutes')")
        .bind(nonce.as_slice())
        .bind(req.account_id)
        .execute(&state.db)
        .await?;
    Ok(Json(ChallengeResp { nonce: BASE64.encode(&nonce) }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginReq {
    account_id: Uuid,
    nonce: String,
    signature: String,
    device: DeviceInfo,
    #[serde(default)]
    totp: Option<String>,
}

pub async fn login(State(state): State<Shared>, headers: HeaderMap, Json(req): Json<LoginReq>) -> ApiResult<Json<SessionResp>> {
    let ip = client_ip(&headers);
    if !state.limits.check(&format!("login:{ip}"), 20, MINUTE) || !state.limits.check(&format!("login-acct:{}", req.account_id), 10, MINUTE) {
        return Err(ApiError::too_many());
    }
    let nonce = b64(&req.nonce)?;
    let used = sqlx::query("DELETE FROM challenges WHERE nonce = $1 AND account_id = $2 AND expires_at > now()")
        .bind(&nonce)
        .bind(req.account_id)
        .execute(&state.db)
        .await?;
    let bad = || ApiError::new(StatusCode::UNAUTHORIZED, "bad_credentials", "E-mail, senha mestra ou Chave Secreta não conferem.");
    if used.rows_affected() == 0 {
        return Err(bad());
    }
    let row: Option<(Vec<u8>, String, i64, Option<Vec<u8>>, Vec<String>)> =
        sqlx::query_as("SELECT auth_public_key, record, record_version, totp_secret, totp_recovery FROM accounts WHERE id = $1")
            .bind(req.account_id)
            .fetch_optional(&state.db)
            .await?;
    let (pk, record, record_version, totp_secret, recovery) = row.ok_or_else(bad)?;
    let vk = VerifyingKey::from_bytes(pk.as_slice().try_into().map_err(|_| bad())?).map_err(|_| bad())?;
    let sig = Signature::from_slice(&b64(&req.signature)?).map_err(|_| bad())?;
    vk.verify_strict(&moco_core::account::login_message(&nonce, &req.account_id), &sig).map_err(|_| {
        bad()
    })?;

    // Two-step verification for new sign-ins.
    if let Some(enc) = totp_secret {
        let code = req.totp.as_deref().map(str::trim).unwrap_or("");
        if code.is_empty() {
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "totp_required", "Digite o código de verificação do seu app autenticador."));
        }
        let secret = state.secrets.open(&enc).ok_or_else(|| ApiError::bad_request("2FA"))?;
        let ok_totp = TotpConfig::parse(std::str::from_utf8(&secret).unwrap_or(""))
            .map(|cfg| {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                [now.saturating_sub(30), now, now + 30].iter().any(|t| cfg.code_at(*t).code == code)
            })
            .unwrap_or(false);
        let code_hash = hex(&Sha256::digest(code.to_uppercase().replace('-', "").as_bytes()));
        let ok_recovery = !ok_totp && recovery.contains(&code_hash);
        if !ok_totp && !ok_recovery {
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "totp_invalid", "Código de verificação incorreto."));
        }
        if ok_recovery {
            sqlx::query("UPDATE accounts SET totp_recovery = array_remove(totp_recovery, $2) WHERE id = $1")
                .bind(req.account_id)
                .bind(&code_hash)
                .execute(&state.db)
                .await?;
        }
    }
    let token = open_session(&state, req.account_id, &req.device).await?;
    audit(&state, req.account_id, "login", Some(req.device.id)).await;
    Ok(Json(SessionResp { token, record, record_version }))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub async fn logout(State(state): State<Shared>, session: Session) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1").bind(&session.token_hash).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- account --------------------------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    id: Uuid,
    name: String,
    platform: String,
    created_at: i64,
    last_seen_at: i64,
    #[sqlx(skip)]
    current: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeResp {
    email: String,
    plan: String,
    two_factor: bool,
    recovery_codes_left: usize,
    created_at: i64,
    devices: Vec<DeviceView>,
    items: i64,
}

pub async fn me(State(state): State<Shared>, session: Session) -> ApiResult<Json<MeResp>> {
    let (email, plan, totp, recovery, created): (String, String, Option<Vec<u8>>, Vec<String>, i64) = sqlx::query_as(
        "SELECT email, plan, totp_secret, totp_recovery, (extract(epoch from created_at) * 1000)::bigint FROM accounts WHERE id = $1",
    )
    .bind(session.account_id)
    .fetch_one(&state.db)
    .await?;
    let mut devices: Vec<DeviceView> = sqlx::query_as(
        "SELECT id, name, platform, (extract(epoch from created_at) * 1000)::bigint AS created_at,
                (extract(epoch from last_seen_at) * 1000)::bigint AS last_seen_at
         FROM devices WHERE account_id = $1 AND revoked_at IS NULL ORDER BY last_seen_at DESC",
    )
    .bind(session.account_id)
    .fetch_all(&state.db)
    .await?;
    for d in devices.iter_mut() {
        d.current = d.id == session.device_id;
    }
    let (items,): (i64,) = sqlx::query_as("SELECT count(*) FROM items WHERE account_id = $1 AND NOT deleted")
        .bind(session.account_id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(MeResp { email, plan, two_factor: totp.is_some(), recovery_codes_left: recovery.len(), created_at: created, devices, items }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PutRecordReq {
    record: String,
    base_version: i64,
    auth_public_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PutRecordResp {
    record_version: i64,
}

/// Password change (or recovery) from a signed-in device: new wrapped key, new salt and
/// login key replace the old ones atomically, and every other session is revoked.
pub async fn put_record(State(state): State<Shared>, session: Session, Json(req): Json<PutRecordReq>) -> ApiResult<Json<PutRecordResp>> {
    let pk = b64(&req.auth_public_key)?;
    check_record(&req.record, session.account_id, &pk)?;
    let mut tx = state.db.begin().await?;
    let updated: Option<(i64,)> = sqlx::query_as(
        "UPDATE accounts SET record = $2, auth_public_key = $3, record_version = record_version + 1
         WHERE id = $1 AND record_version = $4 RETURNING record_version",
    )
    .bind(session.account_id)
    .bind(&req.record)
    .bind(&pk)
    .bind(req.base_version)
    .fetch_optional(&mut *tx)
    .await?;
    let (version,) = updated.ok_or_else(|| ApiError::conflict("A conta mudou em outro dispositivo. Sincronize e tente de novo."))?;
    sqlx::query("DELETE FROM sessions WHERE account_id = $1 AND token_hash <> $2")
        .bind(session.account_id)
        .bind(&session.token_hash)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    audit(&state, session.account_id, "password.changed", Some(session.device_id)).await;
    Ok(Json(PutRecordResp { record_version: version }))
}

pub async fn revoke_device(State(state): State<Shared>, session: Session, Path(id): Path<Uuid>) -> ApiResult<StatusCode> {
    let r = sqlx::query("UPDATE devices SET revoked_at = now() WHERE id = $1 AND account_id = $2 AND revoked_at IS NULL")
        .bind(id)
        .bind(session.account_id)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::not_found());
    }
    sqlx::query("DELETE FROM sessions WHERE device_id = $1").bind(id).execute(&state.db).await?;
    audit(&state, session.account_id, "device.revoked", Some(id)).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_account(State(state): State<Shared>, session: Session) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM accounts WHERE id = $1").bind(session.account_id).execute(&state.db).await?;
    tracing::info!("account deleted");
    Ok(StatusCode::NO_CONTENT)
}

// ---- 2FA --------------------------------------------------------------------------------

#[derive(Serialize)]
pub struct TotpSetupResp {
    secret: String,
    uri: String,
}

/// Starts enrollment: the pending secret is returned once and must be confirmed.
pub async fn totp_setup(State(state): State<Shared>, session: Session) -> ApiResult<Json<TotpSetupResp>> {
    let mut raw = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut raw);
    let secret = data_encoding::BASE32_NOPAD.encode(&raw);
    let email: (String,) = sqlx::query_as("SELECT email FROM accounts WHERE id = $1").bind(session.account_id).fetch_one(&state.db).await?;
    let uri = format!("otpauth://totp/Moc%C3%B3:{}?secret={}&issuer=Moc%C3%B3", email.0, secret);
    Ok(Json(TotpSetupResp { secret, uri }))
}

#[derive(Deserialize)]
pub struct TotpEnableReq {
    secret: String,
    code: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpEnableResp {
    recovery_codes: Vec<String>,
}

pub async fn totp_enable(State(state): State<Shared>, session: Session, Json(req): Json<TotpEnableReq>) -> ApiResult<Json<TotpEnableResp>> {
    let cfg = TotpConfig::parse(&req.secret).map_err(|_| ApiError::bad_request("Chave inválida."))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if ![now.saturating_sub(30), now, now + 30].iter().any(|t| cfg.code_at(*t).code == req.code.trim()) {
        return Err(ApiError::bad_request("Código incorreto. Confira o horário do celular e tente de novo."));
    }
    let mut codes = Vec::new();
    let mut hashes = Vec::new();
    for _ in 0..8 {
        let mut raw = [0u8; 5];
        rand::rngs::OsRng.fill_bytes(&mut raw);
        let c = data_encoding::BASE32_NOPAD.encode(&raw);
        hashes.push(hex(&Sha256::digest(c.as_bytes())));
        codes.push(format!("{}-{}", &c[..4], &c[4..]));
    }
    sqlx::query("UPDATE accounts SET totp_secret = $2, totp_recovery = $3 WHERE id = $1")
        .bind(session.account_id)
        .bind(state.secrets.seal(req.secret.as_bytes()))
        .bind(&hashes)
        .execute(&state.db)
        .await?;
    audit(&state, session.account_id, "2fa.enabled", Some(session.device_id)).await;
    Ok(Json(TotpEnableResp { recovery_codes: codes }))
}

#[derive(Deserialize)]
pub struct TotpDisableReq {
    code: String,
}

pub async fn totp_disable(State(state): State<Shared>, session: Session, Json(req): Json<TotpDisableReq>) -> ApiResult<StatusCode> {
    let (enc,): (Option<Vec<u8>>,) = sqlx::query_as("SELECT totp_secret FROM accounts WHERE id = $1").bind(session.account_id).fetch_one(&state.db).await?;
    let Some(enc) = enc else { return Ok(StatusCode::NO_CONTENT) };
    let secret = state.secrets.open(&enc).ok_or_else(|| ApiError::bad_request("2FA"))?;
    let cfg = TotpConfig::parse(std::str::from_utf8(&secret).unwrap_or("")).map_err(|_| ApiError::bad_request("2FA"))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    if ![now.saturating_sub(30), now, now + 30].iter().any(|t| cfg.code_at(*t).code == req.code.trim()) {
        return Err(ApiError::bad_request("Código incorreto."));
    }
    sqlx::query("UPDATE accounts SET totp_secret = NULL, totp_recovery = '{}' WHERE id = $1").bind(session.account_id).execute(&state.db).await?;
    audit(&state, session.account_id, "2fa.disabled", Some(session.device_id)).await;
    Ok(StatusCode::NO_CONTENT)
}
