//! IPC commands. Every command is async; anything that runs Argon2 goes through
//! `spawn_blocking` so the window never freezes.

use moco_core::account::{Usage, VaultAttrs, VaultInfo};
use moco_core::crypto::{RecoveryCode, SecretKey, SecretString};
use moco_core::generator::{self, Recipe};
use moco_core::model::{FieldType, Item, ItemInput, ItemSummary};
use moco_core::strength::{self, Strength};
use moco_core::totp::{TotpCode, TotpConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::dto::ItemView;
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::settings::Settings;
use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::internal(e.to_string()))?
}

// ---- app / account ------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub initialized: bool,
    pub unlocked: bool,
    /// The Secret Key for this account is stored on this device (no need to type it).
    pub secret_key_on_device: bool,
    pub storage_error: Option<AppError>,
    pub settings: Settings,
    pub debug: bool,
    pub hello_available: bool,
    pub hello_enrolled: bool,
    /// Policy asks for the master password now even though Hello is enrolled.
    pub password_due: bool,
}

#[tauri::command]
pub async fn app_info(app: AppHandle, state: S<'_>) -> AppResult<AppInfo> {
    let settings = state.settings();
    let version = app.package_info().version.to_string();
    let (initialized, unlocked, account_id, hello_enrolled) = match state.account.lock() {
        Ok(g) => match g.as_ref() {
            Some(a) => {
                let st = a.status()?;
                (st.initialized, st.unlocked, st.account_id, st.device_keys.iter().any(|k| k == crate::hello::LABEL))
            }
            None => (false, false, None, false),
        },
        Err(_) => (false, false, None, false),
    };
    let hello_available = tauri::async_runtime::spawn_blocking(crate::hello::available).await.unwrap_or(false);
    Ok(AppInfo {
        version,
        initialized,
        unlocked,
        secret_key_on_device: account_id.is_some_and(|id| state.has_secret_key(id)),
        storage_error: state.open_error.as_ref().map(|e| AppError::new(e.code, e.message.clone())),
        settings,
        debug: cfg!(debug_assertions),
        hello_available,
        hello_enrolled,
        password_due: state.password_due(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedAccount {
    pub secret_key: String,
    pub recovery_code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccountArgs {
    pub password: SecretString,
    #[serde(default)]
    pub vault_name: Option<String>,
}

#[tauri::command]
pub async fn account_create(state: S<'_>, args: CreateAccountArgs) -> AppResult<CreatedAccount> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = SecretKey::generate();
        let name = args.vault_name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "Pessoal".into());
        let (code, account_id) = st.with_account(|acct| {
            let code = acct.create(
                args.password.expose(),
                &sk,
                VaultAttrs { name, description: String::new(), icon: "house".into(), color: "cobalt".into() },
            )?;
            Ok((code, acct.account_id()?))
        })?;
        st.store_secret_key(account_id, &sk)?;
        Ok(CreatedAccount { secret_key: sk.to_display().to_string(), recovery_code: code.to_display().to_string() })
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockArgs {
    pub password: SecretString,
    /// Needed only when this device doesn't hold the Secret Key yet.
    #[serde(default)]
    pub secret_key: Option<SecretString>,
}

fn account_id_of(st: &AppState) -> AppResult<Uuid> {
    st.with_account(|a| a.status()?.account_id.ok_or_else(|| AppError::new("not_initialized", "Nenhum Mocó criado ainda.")))
}

fn check_throttle(st: &AppState) -> AppResult<()> {
    if let Ok(g) = st.retry_after.lock() {
        if let Some(t) = *g {
            let now = Instant::now();
            if t > now {
                let secs = (t - now).as_secs() + 1;
                return Err(AppError::new("throttled", format!("Muitas tentativas. Tente de novo em {secs} s.")));
            }
        }
    }
    Ok(())
}

fn note_failure(st: &AppState) {
    let n = st.failed_unlocks.fetch_add(1, Ordering::SeqCst) + 1;
    if n >= 3 {
        let wait = Duration::from_secs((1u64 << (n - 3).min(5)).min(30));
        if let Ok(mut g) = st.retry_after.lock() {
            *g = Some(Instant::now() + wait);
        }
    }
}

#[tauri::command]
pub async fn account_unlock(app: AppHandle, state: S<'_>, args: UnlockArgs) -> AppResult<()> {
    let st = state.inner().clone();
    check_throttle(&st)?;
    let result = blocking({
        let st = st.clone();
        move || {
            let account_id = account_id_of(&st)?;
            let (sk, typed) = match &args.secret_key {
                Some(typed) if !typed.is_empty() => (SecretKey::parse(typed.expose())?, true),
                _ => (
                    st.load_secret_key(account_id).ok_or_else(|| {
                        AppError::new(
                            "secret_key_missing",
                            "Este computador ainda não tem sua Chave Secreta. Ela está no seu Kit de Emergência.",
                        )
                    })?,
                    false,
                ),
            };
            st.with_account(|a| Ok(a.unlock(args.password.expose(), &sk)?))?;
            st.note_password_unlock();
            if typed {
                // Remember it on this device so it's never asked again here.
                st.store_secret_key(account_id, &sk)?;
            }
            Ok(())
        }
    })
    .await;
    match &result {
        Ok(()) => {
            st.failed_unlocks.store(0, Ordering::SeqCst);
            let _ = app.emit("moco://unlocked", ());
        }
        Err(e) if e.code == "bad_credentials" => note_failure(&st),
        _ => {}
    }
    result
}

/// Locks now: forgets keys, clears a pending copied secret, tells every window.
pub fn lock_now(app: &AppHandle, reason: &str) {
    let st = app.state::<Arc<AppState>>();
    let was_unlocked = st.account.lock().map(|mut g| {
        let unlocked = g.as_ref().is_some_and(|a| a.is_unlocked());
        if let Some(a) = g.as_mut() {
            a.lock();
        }
        unlocked
    });
    // Parsed-but-uncommitted imports hold plaintext: forget them too (D-009).
    if let Ok(mut g) = st.pending_import.lock() {
        *g = None;
    }
    if was_unlocked.unwrap_or(false) {
        let seq = st.clipboard_sequence.swap(0, Ordering::SeqCst);
        if seq != 0 {
            platform::clear_clipboard_if(st.hwnd(), seq);
        }
        let _ = app.emit("moco://locked", reason.to_string());
    }
}

#[tauri::command]
pub async fn account_lock(app: AppHandle) -> AppResult<()> {
    lock_now(&app, "manual");
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordArgs {
    pub password: SecretString,
}

pub(crate) fn device_secret_key(st: &AppState) -> AppResult<SecretKey> {
    let id = account_id_of(st)?;
    st.load_secret_key(id)
        .ok_or_else(|| AppError::new("secret_key_missing", "A Chave Secreta não está guardada neste computador."))
}

/// Shows the Secret Key again (for a new Emergency Kit) after re-entering the password.
#[tauri::command]
pub async fn secret_key_reveal(state: S<'_>, args: PasswordArgs) -> AppResult<String> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = device_secret_key(&st)?;
        st.with_account(|a| Ok(a.verify_password(args.password.expose(), &sk)?))?;
        Ok(sk.to_display().to_string())
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordArgs {
    pub current: SecretString,
    pub next: SecretString,
}

#[tauri::command]
pub async fn password_change(state: S<'_>, args: ChangePasswordArgs) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = device_secret_key(&st)?;
        st.with_account(|a| Ok(a.change_password(args.current.expose(), args.next.expose(), &sk)?))
    })
    .await
}

#[tauri::command]
pub async fn recovery_rotate(state: S<'_>, args: PasswordArgs) -> AppResult<String> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = device_secret_key(&st)?;
        st.with_account(|a| {
            a.verify_password(args.password.expose(), &sk)?;
            Ok(a.rotate_recovery_code(&sk)?.to_display().to_string())
        })
    })
    .await
}

/// Turns the recovery code off (explicit choice after a clear warning).
#[tauri::command]
pub async fn recovery_disable(state: S<'_>) -> AppResult<()> {
    state.with_account(|a| Ok(a.disable_recovery_code()?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverArgs {
    pub recovery_code: SecretString,
    pub new_password: SecretString,
    #[serde(default)]
    pub secret_key: Option<SecretString>,
}

#[tauri::command]
pub async fn account_recover(app: AppHandle, state: S<'_>, args: RecoverArgs) -> AppResult<String> {
    let st = state.inner().clone();
    let out = blocking(move || {
        let id = account_id_of(&st)?;
        let sk = match &args.secret_key {
            Some(t) if !t.is_empty() => SecretKey::parse(t.expose())?,
            _ => st.load_secret_key(id).ok_or_else(|| {
                AppError::new("secret_key_missing", "Para recuperar, precisamos também da Chave Secreta do seu Kit de Emergência.")
            })?,
        };
        let code = RecoveryCode::parse(args.recovery_code.expose())?;
        let fresh = st.with_account(|a| Ok(a.recover(&code, args.new_password.expose(), &sk)?))?;
        st.store_secret_key(id, &sk)?;
        Ok(fresh.to_display().to_string())
    })
    .await?;
    let _ = app.emit("moco://unlocked", ());
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityStatus {
    pub recovery_enabled: bool,
    pub recovery_created_at: Option<i64>,
    pub password_changed_at: Option<i64>,
    pub created_at: Option<i64>,
    pub device_keys: Vec<String>,
}

#[tauri::command]
pub async fn account_security(state: S<'_>) -> AppResult<SecurityStatus> {
    state.with_account(|a| {
        let s = a.status()?;
        Ok(SecurityStatus {
            recovery_enabled: s.recovery_enabled,
            recovery_created_at: s.recovery_created_at,
            password_changed_at: s.password_changed_at,
            created_at: s.created_at,
            device_keys: s.device_keys,
        })
    })
}

// ---- Windows Hello ------------------------------------------------------------------

#[tauri::command]
pub async fn hello_enable(state: S<'_>, args: PasswordArgs) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let sk = device_secret_key(&st)?;
        let account = account_id_of(&st)?;
        st.with_account(|a| Ok(a.verify_password(args.password.expose(), &sk)?))?;
        let mut dev = st.device();
        let salt = crate::hello::new_salt();
        let key = crate::hello::enroll(account, &salt)?;
        st.with_account(|a| Ok(a.enroll_device_key(crate::hello::LABEL, &key)?))?;
        dev.hello_salt = salt.to_vec();
        st.save_device(&dev)
    })
    .await
}

#[tauri::command]
pub async fn hello_disable(state: S<'_>) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || {
        let account = account_id_of(&st)?;
        st.with_account(|a| Ok(a.remove_device_key(crate::hello::LABEL)?))?;
        crate::hello::remove(account);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn account_unlock_hello(app: AppHandle, state: S<'_>) -> AppResult<()> {
    let st = state.inner().clone();
    if st.password_due() {
        return Err(AppError::new("password_due", "Faz um tempo que você não digita a senha mestra. Digite-a desta vez, para não esquecê-la."));
    }
    blocking(move || {
        let account = account_id_of(&st)?;
        let salt = st.device().hello_salt;
        if salt.is_empty() {
            return Err(AppError::new("hello_missing", "O Windows Hello não está ativado neste computador."));
        }
        let key = crate::hello::unlock_key(account, &salt)?;
        st.with_account(|a| {
            a.unlock_with_device_key(crate::hello::LABEL, &key).map_err(|_| {
                AppError::new("hello_failed", "O Windows Hello não conseguiu abrir o Mocó. Use a senha mestra e ative de novo.")
            })
        })
    })
    .await?;
    let _ = app.emit("moco://unlocked", ());
    Ok(())
}

// ---- vaults -------------------------------------------------------------------------

#[tauri::command]
pub async fn vaults_list(state: S<'_>) -> AppResult<Vec<VaultInfo>> {
    state.with_account(|a| Ok(a.vaults()?))
}

#[tauri::command]
pub async fn vault_create(state: S<'_>, attrs: VaultAttrs) -> AppResult<VaultInfo> {
    state.with_account(|a| Ok(a.create_vault(attrs)?))
}

#[tauri::command]
pub async fn vault_update(state: S<'_>, id: Uuid, attrs: VaultAttrs) -> AppResult<VaultInfo> {
    state.with_account(|a| Ok(a.update_vault(id, attrs)?))
}

#[tauri::command]
pub async fn vault_delete(state: S<'_>, id: Uuid, move_to: Option<Uuid>) -> AppResult<()> {
    state.with_account(|a| Ok(a.delete_vault(id, move_to)?))
}

// ---- items --------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemsPayload {
    pub items: Vec<ItemSummary>,
    pub usage: HashMap<Uuid, Usage>,
}

#[tauri::command]
pub async fn items_list(state: S<'_>) -> AppResult<ItemsPayload> {
    state.with_account(|a| Ok(ItemsPayload { items: a.summaries()?, usage: a.usage()? }))
}

#[tauri::command]
pub async fn item_get(state: S<'_>, id: Uuid) -> AppResult<ItemView> {
    state.with_account(|a| Ok(ItemView::from(&a.item(id)?)))
}

/// Full item including concealed values — only for the editor.
#[tauri::command]
pub async fn item_get_for_edit(state: S<'_>, id: Uuid) -> AppResult<Item> {
    state.with_account(|a| Ok(a.item(id)?))
}

#[tauri::command]
pub async fn item_create(state: S<'_>, vault_id: Uuid, input: ItemInput) -> AppResult<ItemView> {
    state.with_account(|a| Ok(ItemView::from(&a.create_item(vault_id, input)?)))
}

#[tauri::command]
pub async fn item_update(state: S<'_>, id: Uuid, input: ItemInput) -> AppResult<ItemView> {
    state.with_account(|a| Ok(ItemView::from(&a.update_item(id, input)?)))
}

#[tauri::command]
pub async fn item_set_favorite(state: S<'_>, id: Uuid, favorite: bool) -> AppResult<()> {
    state.with_account(|a| a.set_favorite(id, favorite).map(|_| ()).map_err(Into::into))
}

#[tauri::command]
pub async fn item_set_archived(state: S<'_>, id: Uuid, archived: bool) -> AppResult<()> {
    state.with_account(|a| a.set_archived(id, archived).map(|_| ()).map_err(Into::into))
}

#[tauri::command]
pub async fn item_trash(state: S<'_>, id: Uuid) -> AppResult<()> {
    state.with_account(|a| a.trash_item(id).map(|_| ()).map_err(Into::into))
}

#[tauri::command]
pub async fn item_restore(state: S<'_>, id: Uuid) -> AppResult<()> {
    state.with_account(|a| a.restore_item(id).map(|_| ()).map_err(Into::into))
}

#[tauri::command]
pub async fn item_purge(state: S<'_>, id: Uuid) -> AppResult<()> {
    state.with_account(|a| Ok(a.purge_item(id)?))
}

#[tauri::command]
pub async fn trash_empty(state: S<'_>) -> AppResult<usize> {
    state.with_account(|a| Ok(a.empty_trash()?))
}

#[tauri::command]
pub async fn item_move(state: S<'_>, id: Uuid, vault_id: Uuid) -> AppResult<()> {
    state.with_account(|a| a.move_item(id, vault_id).map(|_| ()).map_err(Into::into))
}

#[tauri::command]
pub async fn item_duplicate(state: S<'_>, id: Uuid) -> AppResult<ItemView> {
    state.with_account(|a| Ok(ItemView::from(&a.duplicate_item(id)?)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionView {
    pub revision: u64,
    pub saved_at: i64,
    pub item: ItemView,
}

#[tauri::command]
pub async fn item_history(state: S<'_>, id: Uuid) -> AppResult<Vec<VersionView>> {
    state.with_account(|a| {
        Ok(a.history(id)?
            .iter()
            .map(|v| VersionView { revision: v.revision, saved_at: v.saved_at, item: ItemView::from(&v.item) })
            .collect())
    })
}

#[tauri::command]
pub async fn item_restore_version(state: S<'_>, id: Uuid, revision: u64) -> AppResult<ItemView> {
    state.with_account(|a| Ok(ItemView::from(&a.restore_version(id, revision)?)))
}

#[tauri::command]
pub async fn item_touch(state: S<'_>, id: Uuid) -> AppResult<()> {
    state.with_account(|a| Ok(a.touch(id)?))
}

// ---- field access -------------------------------------------------------------------

/// Resolves a field reference: a field id, "notes", or "history:<n>".
fn field_value(item: &Item, field: &str) -> AppResult<(Zeroizing<String>, FieldType)> {
    if field == "notes" {
        return Ok((Zeroizing::new(item.details.notes.expose().to_string()), FieldType::Multiline));
    }
    if let Some(n) = field.strip_prefix("history:") {
        let idx: usize = n.parse().map_err(|_| AppError::new("invalid", "Referência inválida."))?;
        let h = item.details.password_history.get(idx).ok_or_else(|| AppError::new("not_found", "Versão não encontrada."))?;
        return Ok((Zeroizing::new(h.value.expose().to_string()), FieldType::Concealed));
    }
    let f = item.details.field(field).ok_or_else(|| AppError::new("not_found", "Campo não encontrado."))?;
    Ok((Zeroizing::new(f.value.expose().to_string()), f.kind))
}

#[tauri::command]
pub async fn field_reveal(state: S<'_>, item_id: Uuid, field: String) -> AppResult<String> {
    state.with_account(|a| {
        let item = a.item(item_id)?;
        let (v, _) = field_value(&item, &field)?;
        Ok(v.to_string())
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    /// Seconds until the clipboard is cleared (0 = not cleared automatically).
    pub clears_in: u32,
}

fn copy_and_schedule(app: &AppHandle, st: &Arc<AppState>, text: &str, sensitive: bool) -> AppResult<CopyResult> {
    let seq = platform::copy_text(st.hwnd(), text, sensitive).map_err(|e| AppError::new("clipboard", format!("Não deu para copiar: {e}.")))?;
    let clears_in = if sensitive { st.settings().clipboard_clear_seconds } else { 0 };
    if clears_in > 0 {
        st.clipboard_sequence.store(seq, Ordering::SeqCst);
        let generation = st.clipboard_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let st = st.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(clears_in as u64)).await;
            if st.clipboard_generation.load(Ordering::SeqCst) == generation {
                let seq = st.clipboard_sequence.swap(0, Ordering::SeqCst);
                if seq != 0 && platform::clear_clipboard_if(st.hwnd(), seq) {
                    let _ = app.emit("moco://clipboard-cleared", ());
                }
            }
        });
    }
    Ok(CopyResult { clears_in })
}

/// Copies a field (or the current TOTP code) straight from Rust to the clipboard.
#[tauri::command]
pub async fn field_copy(app: AppHandle, state: S<'_>, item_id: Uuid, field: String) -> AppResult<CopyResult> {
    let st = state.inner().clone();
    let (value, sensitive) = st.with_account(|a| {
        let item = a.item(item_id)?;
        let (v, kind) = field_value(&item, &field)?;
        let _ = a.touch(item_id);
        if kind == FieldType::Totp {
            let cfg = TotpConfig::parse(&v)?;
            return Ok((Zeroizing::new(cfg.now().code), true));
        }
        Ok((v, kind.is_concealed() || matches!(kind, FieldType::CardNumber | FieldType::Cpf)))
    })?;
    if value.is_empty() {
        return Err(AppError::new("empty", "Esse campo está vazio."));
    }
    copy_and_schedule(&app, &st, &value, sensitive)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyTextArgs {
    pub text: SecretString,
    #[serde(default)]
    pub sensitive: bool,
}

/// Copies arbitrary text (e.g. a freshly generated password not saved yet).
#[tauri::command]
pub async fn copy_text(app: AppHandle, state: S<'_>, args: CopyTextArgs) -> AppResult<CopyResult> {
    let st = state.inner().clone();
    copy_and_schedule(&app, &st, args.text.expose(), args.sensitive)
}

#[tauri::command]
pub async fn totp_now(state: S<'_>, item_id: Uuid, field: String) -> AppResult<TotpCode> {
    state.with_account(|a| {
        let item = a.item(item_id)?;
        let (v, _) = field_value(&item, &field)?;
        Ok(TotpConfig::parse(&v)?.now())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpPreviewArgs {
    pub secret: SecretString,
}

/// Validates a 2FA secret/URI typed in the editor and returns the current code.
#[tauri::command]
pub async fn totp_preview(args: TotpPreviewArgs) -> AppResult<TotpCode> {
    Ok(TotpConfig::parse(args.secret.expose())?.now())
}

// ---- generator / strength -----------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedView {
    pub value: String,
    pub entropy_bits: f64,
    pub strength: Strength,
}

#[tauri::command]
pub async fn generator_generate(recipe: Recipe) -> AppResult<GeneratedView> {
    let g = generator::generate(&recipe)?;
    let strength = strength::estimate(g.value.expose(), &[]);
    Ok(GeneratedView { value: g.value.expose().to_string(), entropy_bits: g.entropy_bits, strength })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrengthArgs {
    pub password: SecretString,
    #[serde(default)]
    pub context: Vec<String>,
}

#[tauri::command]
pub async fn strength_estimate(args: StrengthArgs) -> AppResult<Strength> {
    let ctx: Vec<&str> = args.context.iter().map(String::as_str).collect();
    Ok(strength::estimate(args.password.expose(), &ctx))
}

// ---- security center ---------------------------------------------------------------

#[tauri::command]
pub async fn health_report(state: S<'_>) -> AppResult<moco_core::health::HealthReport> {
    let st = state.inner().clone();
    blocking(move || {
        let items = st.with_account(|a| Ok(a.all_items()?))?;
        Ok(moco_core::health::analyze(&items))
    })
    .await
}

#[tauri::command]
pub async fn breach_check(state: S<'_>) -> AppResult<crate::breach::BreachReport> {
    let st = state.inner().clone();
    if !st.settings().breach_check {
        return Err(AppError::new("disabled", "A verificação de vazamentos está desligada."));
    }
    blocking(move || {
        let items = st.with_account(|a| Ok(a.all_items()?))?;
        crate::breach::check(&items)
    })
    .await
}

// ---- settings / system --------------------------------------------------------------

#[tauri::command]
pub async fn settings_get(state: S<'_>) -> AppResult<Settings> {
    Ok(state.settings())
}

#[tauri::command]
pub async fn settings_update(app: AppHandle, state: S<'_>, settings: Settings) -> AppResult<Settings> {
    let clean = settings.sanitized();
    clean.save(&state.dir)?;
    if let Ok(mut g) = state.settings.write() {
        *g = clean.clone();
    }
    crate::apply_settings(&app, &clean);
    let _ = app.emit("moco://settings", clean.clone());
    Ok(clean)
}

/// Opens an http(s) URL in the default browser. Nothing else is allowed through.
#[tauri::command]
pub async fn open_url(app: AppHandle, url: String) -> AppResult<()> {
    let trimmed = url.trim();
    let full = if trimmed.contains("://") { trimmed.to_string() } else { format!("https://{trimmed}") };
    let lower = full.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(AppError::new("invalid", "Só abrimos endereços da web (http ou https)."));
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(full, None::<&str>).map_err(|e| AppError::internal(e.to_string()))
}
