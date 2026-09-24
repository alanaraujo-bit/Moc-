//! Account and sync client. Talks to the Mocó server with `ureq` from Rust (the webview's
//! CSP stays closed to the network). Only ciphertext and signatures leave this device.

use data_encoding::BASE64;
use moco_core::account::sync::{PullResponse, PushRequest, PushResponse, SyncOutcome};
use moco_core::account::{derive_password_keys, login_message, Account};
use moco_core::crypto::kdf::KdfParams;
use moco_core::crypto::{Purpose, SecretKey, SecretString};
use moco_core::model::now_ms;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;

const CLOUD_META: &str = "cloud";
const TOKEN_META: &str = "cloud-token";
const DEFAULT_SERVER: &str = "https://server-production-975b.up.railway.app";

pub fn server_url() -> String {
    if cfg!(debug_assertions) {
        if let Ok(s) = std::env::var("MOCO_SERVER") {
            return s.trim_end_matches('/').to_string();
        }
    }
    DEFAULT_SERVER.to_string()
}

/// Non-secret sync settings, kept in the store's meta table.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloudConfig {
    pub email: String,
    pub server: String,
    pub last_sync_at: i64,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Default)]
pub struct CloudRuntime {
    pub syncing: AtomicBool,
    pub last_outcome: Mutex<Option<SyncOutcome>>,
}

// ---- HTTP ---------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(concat!("Moco-Desktop/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn offline() -> AppError {
    AppError::new("offline", "Sem conexão com o servidor do Mocó. Seus dados continuam aqui; a sincronização volta sozinha.")
}

fn call(method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> AppResult<Value> {
    let url = format!("{}{}", server_url(), path);
    let a = agent();
    let mut resp = match (method, body) {
        ("GET", _) => {
            let mut r = a.get(&url);
            if let Some(t) = token {
                r = r.header("Authorization", &format!("Bearer {t}"));
            }
            r.call()
        }
        ("DELETE", _) => {
            let mut r = a.delete(&url);
            if let Some(t) = token {
                r = r.header("Authorization", &format!("Bearer {t}"));
            }
            r.call()
        }
        (m, b) => {
            let mut r = if m == "PUT" { a.put(&url) } else { a.post(&url) };
            if let Some(t) = token {
                r = r.header("Authorization", &format!("Bearer {t}"));
            }
            r.header("Content-Type", "application/json").send(serde_json::to_vec(&b.unwrap_or(json!({}))).expect("json"))
        }
    }
    .map_err(|_| offline())?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let v: Value = if text.is_empty() { Value::Null } else { serde_json::from_str(&text).unwrap_or(Value::Null) };
    if (200..300).contains(&status) {
        return Ok(v);
    }
    let code = v["code"].as_str().unwrap_or("server");
    let message = v["message"].as_str().unwrap_or("O servidor recusou o pedido.").to_string();
    let code: &'static str = match code {
        "unauthorized" => "cloud_unauthorized",
        "bad_credentials" => "bad_credentials",
        "totp_required" => "totp_required",
        "totp_invalid" => "totp_invalid",
        "conflict" => "conflict",
        "forbidden" => "forbidden",
        "throttled" => "throttled",
        _ => "server",
    };
    Err(AppError::new(code, message))
}

fn call_bytes(method: &str, path: &str, token: &str, body: Option<&[u8]>) -> AppResult<Vec<u8>> {
    let url = format!("{}{}", server_url(), path);
    let a = agent();
    let auth = format!("Bearer {token}");
    let mut resp = match method {
        "PUT" => a.put(&url).header("Authorization", &auth).header("Content-Type", "application/octet-stream").send(body.unwrap_or(&[])),
        "DELETE" => a.delete(&url).header("Authorization", &auth).call(),
        _ => a.get(&url).header("Authorization", &auth).call(),
    }
    .map_err(|_| offline())?;
    let status = resp.status().as_u16();
    let bytes = resp.body_mut().with_config().limit(40 * 1024 * 1024).read_to_vec().map_err(|_| offline())?;
    match status {
        200..=299 => Ok(bytes),
        404 => Err(AppError::new("not_found", "Este anexo ainda não chegou ao servidor. Abra o Mocó no outro dispositivo para ele terminar de enviar.")),
        401 => Err(AppError::new("cloud_unauthorized", "Sessão expirada.")),
        413 => Err(AppError::new("quota", "O espaço para anexos da sua conta acabou.")),
        _ => Err(AppError::new("server", "O servidor recusou o anexo.")),
    }
}

/// Uploads new attachment blobs and applies attachment removals.
fn sync_attachments(st: &AppState, tok: &str) -> AppResult<()> {
    for id in st.with_account(|a| Ok(a.store().attachment_deletes()?))? {
        match call_bytes("DELETE", &format!("/v1/attachments/{id}"), tok, None) {
            Ok(_) | Err(AppError { code: "not_found", .. }) => st.with_account(|a| Ok(a.store().clear_attachment_delete(id)?))?,
            Err(e) => return Err(e),
        }
    }
    loop {
        let batch = st.with_account(|a| Ok(a.store().attachments_to_upload()?))?;
        if batch.is_empty() {
            break;
        }
        for (id, item, blob) in batch {
            call_bytes("PUT", &format!("/v1/attachments/{id}?item={item}"), tok, Some(&blob))?;
            st.with_account(|a| Ok(a.store().mark_attachment_uploaded(id)?))?;
        }
    }
    Ok(())
}

/// Downloads an attachment blob synced from another device (it stays encrypted here).
pub fn fetch_attachment(st: &AppState, item_id: Uuid, att_id: Uuid) -> AppResult<()> {
    let tok = st.with_account(|a| token(a))?.ok_or_else(|| AppError::new("not_found", "Este anexo não está neste computador."))?;
    let blob = call_bytes("GET", &format!("/v1/attachments/{att_id}"), &tok, None)?;
    st.with_account(|a| {
        let vault = a.item(item_id)?.vault_id;
        Ok(a.store().put_attachment_state(att_id, item_id, vault, &blob, now_ms(), true)?)
    })
}

fn device_info(st: &AppState) -> AppResult<Value> {
    let mut d = st.device();
    if d.device_id.is_nil() {
        d.device_id = Uuid::new_v4();
        st.save_device(&d)?;
    }
    let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Computador".into());
    Ok(json!({ "id": d.device_id, "name": name, "platform": "windows" }))
}

// ---- local config / token ---------------------------------------------------------------

fn config(acct: &Account) -> AppResult<Option<CloudConfig>> {
    Ok(acct.store().meta_get(CLOUD_META)?.and_then(|raw| serde_json::from_slice(&raw).ok()))
}

fn save_config(acct: &Account, c: &CloudConfig) -> AppResult<()> {
    Ok(acct.store().meta_set(CLOUD_META, &serde_json::to_vec(c).expect("serializes"))?)
}

fn token(acct: &Account) -> AppResult<Option<String>> {
    let id = acct.account_id()?;
    match acct.store().meta_get(TOKEN_META)? {
        Some(blob) => {
            let pt = acct.open_with_account_key(Purpose::SyncToken, id, &blob)?;
            Ok(Some(String::from_utf8_lossy(&pt).to_string()))
        }
        None => Ok(None),
    }
}

fn save_token(acct: &Account, t: &str) -> AppResult<()> {
    let id = acct.account_id()?;
    let blob = acct.seal_with_account_key(Purpose::SyncToken, id, t.as_bytes())?;
    Ok(acct.store().meta_set(TOKEN_META, &blob)?)
}

fn forget(acct: &Account) -> AppResult<()> {
    acct.store().meta_delete(TOKEN_META)?;
    acct.store().meta_delete(CLOUD_META)?;
    acct.reset_sync_state()?;
    Ok(())
}

// ---- sync ---------------------------------------------------------------------------------

/// One full sync round. Network calls happen without holding the account lock.
pub fn sync_once(st: &AppState) -> AppResult<SyncOutcome> {
    let tok = st.with_account(|a| token(a))?.ok_or_else(|| AppError::new("not_connected", "Sincronização desativada."))?;
    let mut total = SyncOutcome::default();
    for _ in 0..4 {
        let (upto, req): (i64, PushRequest) = st.with_account(|a| Ok(a.sync_collect()?))?;
        if !req.is_empty() {
            let resp: PushResponse = serde_json::from_value(call("POST", "/v1/sync/push", Some(&tok), Some(serde_json::to_value(&req).expect("json")))?)
                .map_err(|_| AppError::new("server", "Resposta inesperada do servidor."))?;
            let o = st.with_account(|a| Ok(a.sync_apply_push(upto, &resp)?))?;
            total.pushed += o.pushed;
            total.conflicts_resolved += o.conflicts_resolved;
            total.rejected += o.rejected;
        }
        let (cursor, rv) = st.with_account(|a| Ok((a.sync_cursor()?, a.record_version()?)))?;
        let pull: PullResponse = serde_json::from_value(call("GET", &format!("/v1/sync/pull?since={cursor}&recordVersion={rv}"), Some(&tok), None)?)
            .map_err(|_| AppError::new("server", "Resposta inesperada do servidor."))?;
        let more = pull.cursor > cursor && (pull.items.len() >= 2000);
        let o = st.with_account(|a| Ok(a.sync_apply_pull(&pull)?))?;
        total.pulled += o.pulled;
        total.conflicts_resolved += o.conflicts_resolved;
        total.rejected += o.rejected;
        let pending = st.with_account(|a| Ok(!a.sync_collect()?.1.is_empty()))?;
        if !pending && !more {
            break;
        }
    }
    sync_attachments(st, &tok)?;
    st.with_account(|a| {
        if let Some(mut c) = config(a)? {
            c.last_sync_at = now_ms();
            c.last_error = None;
            save_config(a, &c)?;
        }
        Ok(())
    })?;
    Ok(total)
}

fn run_sync(app: &AppHandle) -> AppResult<SyncOutcome> {
    let st = app.state::<Arc<AppState>>();
    let rt = &st.cloud;
    if rt.syncing.swap(true, Ordering::SeqCst) {
        return Ok(SyncOutcome::default());
    }
    let res = sync_once(&st);
    rt.syncing.store(false, Ordering::SeqCst);
    match &res {
        Ok(o) => {
            if let Ok(mut g) = rt.last_outcome.lock() {
                *g = Some(o.clone());
            }
            if o.pulled > 0 || o.conflicts_resolved > 0 {
                let _ = app.emit("moco://synced", o.clone());
            }
        }
        Err(e) => {
            if e.code == "cloud_unauthorized" || e.code == "forbidden" {
                let _ = st.with_account(|a| forget(a));
                let _ = app.emit("moco://cloud-signed-out", e.message.clone());
            } else {
                let msg = e.message.clone();
                let _ = st.with_account(|a| {
                    if let Some(mut c) = config(a)? {
                        c.last_error = Some(msg);
                        save_config(a, &c)?;
                    }
                    Ok(())
                });
            }
        }
    }
    res
}

/// Background loop: syncs within seconds of local changes, and every 20 s otherwise.
pub fn spawn_loop(app: AppHandle) {
    std::thread::Builder::new()
        .name("moco-sync".into())
        .spawn(move || {
            let mut last_full = std::time::Instant::now() - Duration::from_secs(3600);
            loop {
                std::thread::sleep(Duration::from_secs(3));
                let st = app.state::<Arc<AppState>>();
                let ready = st
                    .account
                    .lock()
                    .ok()
                    .and_then(|g| g.as_ref().map(|a| a.is_unlocked() && a.store().meta_get(TOKEN_META).ok().flatten().is_some()))
                    .unwrap_or(false);
                if !ready {
                    continue;
                }
                let dirty = st
                    .with_account(|a| {
                        let st = a.store();
                        Ok(st.outbox_len()? > 0 || !st.attachments_to_upload()?.is_empty() || !st.attachment_deletes()?.is_empty())
                    })
                    .unwrap_or(false);
                if dirty || last_full.elapsed() > Duration::from_secs(20) {
                    last_full = std::time::Instant::now();
                    let _ = run_sync(&app);
                }
            }
        })
        .expect("spawn sync thread");
}

// ---- commands -------------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    pub connected: bool,
    pub email: Option<String>,
    pub server: String,
    pub last_sync_at: i64,
    pub last_error: Option<String>,
    pub pending: u64,
    pub syncing: bool,
}

#[tauri::command]
pub async fn cloud_status(state: S<'_>) -> AppResult<CloudStatus> {
    let syncing = state.cloud.syncing.load(Ordering::SeqCst);
    state.with_account(|a| {
        let c = config(a)?;
        let connected = a.store().meta_get(TOKEN_META)?.is_some();
        Ok(CloudStatus {
            connected,
            email: c.as_ref().map(|c| c.email.clone()),
            server: server_url(),
            last_sync_at: c.as_ref().map(|c| c.last_sync_at).unwrap_or(0),
            last_error: c.and_then(|c| c.last_error),
            pending: a.store().outbox_len()?,
            syncing,
        })
    })
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::internal(e.to_string()))?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignupArgs {
    pub email: String,
    pub password: SecretString,
}

/// Turns on sync for the account on this computer (creates the server account).
#[tauri::command]
pub async fn cloud_signup(app: AppHandle, state: S<'_>, args: SignupArgs) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = crate::commands::device_secret_key(&st)?;
        // Re-deriving proves the password and refreshes the login key for this session.
        st.with_account(|a| Ok(a.verify_password(args.password.expose(), &sk)?))?;
        let (record, account_id, pk) = st.with_account(|a| {
            let pk = a.login_public_key()?.ok_or_else(|| AppError::new("invalid", "Tranque e abra o Mocó com a senha mestra uma vez antes de ativar a sincronização."))?;
            Ok((a.record_json()?, a.account_id()?, pk))
        })?;
        let body = json!({
            "email": args.email.trim(),
            "accountId": account_id,
            "record": record,
            "authPublicKey": BASE64.encode(&pk),
            "device": device_info(&st)?,
        });
        let resp = call("POST", "/v1/register", None, Some(body))?;
        let tok = resp["token"].as_str().ok_or_else(|| AppError::new("server", "Resposta inesperada do servidor."))?.to_string();
        st.with_account(|a| {
            save_token(a, &tok)?;
            save_config(a, &CloudConfig { email: args.email.trim().to_lowercase(), server: server_url(), last_sync_at: 0, last_error: None })?;
            a.set_record_version(1)?;
            let _ = a.store().log_event("cloud.connected", None);
            Ok(())
        })?;
        Ok(())
    })
    .await?;
    let _ = tauri::async_runtime::spawn_blocking(move || run_sync(&app)).await;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SigninArgs {
    pub email: String,
    pub password: SecretString,
    pub secret_key: SecretString,
    #[serde(default)]
    pub totp: Option<String>,
}

/// New device: downloads the encrypted account and unlocks it here.
#[tauri::command]
pub async fn cloud_signin(app: AppHandle, state: S<'_>, args: SigninArgs) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = SecretKey::parse(args.secret_key.expose())?;
        let pre = call("POST", "/v1/prelogin", None, Some(json!({ "email": args.email.trim() })))?;
        let account_id: Uuid = serde_json::from_value(pre["accountId"].clone()).map_err(|_| AppError::new("server", "Resposta inesperada."))?;
        let kdf: KdfParams = serde_json::from_value(pre["kdf"].clone()).map_err(|_| AppError::new("server", "Resposta inesperada."))?;
        // Refuses weak parameters a hostile server might send.
        kdf.validate()?;
        let keys = derive_password_keys(args.password.expose(), &sk, &kdf, &account_id)?;
        let ch = call("POST", "/v1/login/challenge", None, Some(json!({ "accountId": account_id })))?;
        let nonce = BASE64.decode(ch["nonce"].as_str().unwrap_or("").as_bytes()).map_err(|_| AppError::new("server", "Resposta inesperada."))?;
        let sig = keys.login.sign(&login_message(&nonce, &account_id));
        let resp = call(
            "POST",
            "/v1/login",
            None,
            Some(json!({
                "accountId": account_id,
                "nonce": BASE64.encode(&nonce),
                "signature": BASE64.encode(&sig),
                "device": device_info(&st)?,
                "totp": args.totp,
            })),
        )?;
        let tok = resp["token"].as_str().ok_or_else(|| AppError::new("server", "Resposta inesperada."))?.to_string();
        let record = resp["record"].as_str().ok_or_else(|| AppError::new("server", "Resposta inesperada."))?.to_string();
        let rv = resp["recordVersion"].as_u64().unwrap_or(1);
        st.with_account(|a| {
            a.install_record(&record, account_id, rv)?;
            a.unlock(args.password.expose(), &sk).map_err(|e| match e {
                moco_core::CoreError::BadCredentials => AppError::new("bad_credentials", "Senha mestra ou Chave Secreta não conferem."),
                other => other.into(),
            })?;
            save_token(a, &tok)?;
            save_config(a, &CloudConfig { email: args.email.trim().to_lowercase(), server: server_url(), last_sync_at: 0, last_error: None })?;
            Ok(())
        })?;
        st.store_secret_key(account_id, &sk)?;
        st.note_password_unlock();
        Ok(())
    })
    .await?;
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || run_sync(&app2)).await.map_err(|e| AppError::internal(e.to_string()))??;
    let _ = app.emit("moco://unlocked", ());
    Ok(())
}

#[tauri::command]
pub async fn cloud_sync_now(app: AppHandle) -> AppResult<SyncOutcome> {
    tauri::async_runtime::spawn_blocking(move || run_sync(&app)).await.map_err(|e| AppError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn cloud_signout(state: S<'_>) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let tok = st.with_account(|a| token(a))?;
        if let Some(t) = tok {
            let _ = call("POST", "/v1/logout", Some(&t), None);
        }
        st.with_account(|a| forget(a))
    })
    .await
}

fn authed(st: &AppState) -> AppResult<String> {
    st.with_account(|a| token(a))?.ok_or_else(|| AppError::new("not_connected", "Sincronização desativada."))
}

#[tauri::command]
pub async fn cloud_me(state: S<'_>) -> AppResult<Value> {
    let st = state.inner().clone();
    blocking(move || {
        let t = authed(&st)?;
        call("GET", "/v1/me", Some(&t), None)
    })
    .await
}

#[tauri::command]
pub async fn cloud_revoke_device(state: S<'_>, id: Uuid) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let t = authed(&st)?;
        call("DELETE", &format!("/v1/devices/{id}"), Some(&t), None).map(|_| ())
    })
    .await
}

#[tauri::command]
pub async fn cloud_totp_setup(state: S<'_>) -> AppResult<Value> {
    let st = state.inner().clone();
    blocking(move || {
        let t = authed(&st)?;
        call("POST", "/v1/2fa/setup", Some(&t), None)
    })
    .await
}

#[tauri::command]
pub async fn cloud_totp_enable(state: S<'_>, secret: String, code: String) -> AppResult<Value> {
    let st = state.inner().clone();
    blocking(move || {
        let t = authed(&st)?;
        call("POST", "/v1/2fa/enable", Some(&t), Some(json!({ "secret": secret, "code": code })))
    })
    .await
}

#[tauri::command]
pub async fn cloud_totp_disable(state: S<'_>, code: String) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let t = authed(&st)?;
        call("POST", "/v1/2fa/disable", Some(&t), Some(json!({ "code": code }))).map(|_| ())
    })
    .await
}

/// After a local password change: push the new record (and login key) atomically.
pub fn push_record(st: &AppState) -> AppResult<()> {
    let Some(t) = st.with_account(|a| token(a))? else { return Ok(()) };
    let (record, base, pk) = st.with_account(|a| {
        Ok((a.record_json()?, a.record_version()?, a.login_public_key()?.ok_or_else(|| AppError::internal("sem chave de login"))?))
    })?;
    let resp = call("PUT", "/v1/record", Some(&t), Some(json!({ "record": record, "baseVersion": base, "authPublicKey": BASE64.encode(&pk) })))?;
    let v = resp["recordVersion"].as_u64().unwrap_or(base + 1);
    st.with_account(|a| Ok(a.set_record_version(v)?))
}

#[tauri::command]
pub async fn cloud_delete_account(state: S<'_>, password: SecretString) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = crate::commands::device_secret_key(&st)?;
        st.with_account(|a| Ok(a.verify_password(password.expose(), &sk)?))?;
        let t = authed(&st)?;
        call("DELETE", "/v1/account", Some(&t), None)?;
        st.with_account(|a| forget(a))
    })
    .await
}
