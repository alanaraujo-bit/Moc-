//! Process-wide state. The account (and therefore every key) lives only here, in Rust.

use moco_core::account::Account;
use moco_core::crypto::SecretKey;
use moco_core::store::Store;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::Instant;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};
use crate::platform;
use crate::settings::Settings;

const SECRET_KEY_FILE: &str = "secret-key.bin";
const DPAPI_ENTROPY: &[u8] = b"moco/v1/secret-key";

pub struct AppState {
    pub dir: PathBuf,
    pub account: Mutex<Option<Account>>,
    /// Why the store could not be opened (corrupt file, newer format…), if it failed.
    pub open_error: Option<AppError>,
    pub settings: RwLock<Settings>,
    pub main_hwnd: AtomicIsize,
    /// Clipboard sequence number we wrote, cleared later only if unchanged.
    pub clipboard_sequence: AtomicU32,
    /// Bumped on every copy so older clear timers become no-ops.
    pub clipboard_generation: AtomicU64,
    pub failed_unlocks: AtomicU32,
    pub retry_after: Mutex<Option<Instant>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSecretKey {
    account_id: Uuid,
    #[serde(with = "b64")]
    blob: Vec<u8>,
}

mod b64 {
    use data_encoding::BASE64;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(b: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(b))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        BASE64.decode(s.as_bytes()).map_err(serde::de::Error::custom)
    }
}

impl AppState {
    pub fn new(dir: PathBuf) -> Self {
        let settings = Settings::load(&dir);
        let (account, open_error) = match Store::open(&dir.join("vault.db")) {
            Ok(store) => (Some(Account::new(store)), None),
            Err(e) => (None, Some(AppError::from(e))),
        };
        Self {
            dir,
            account: Mutex::new(account),
            open_error,
            settings: RwLock::new(settings),
            main_hwnd: AtomicIsize::new(0),
            clipboard_sequence: AtomicU32::new(0),
            clipboard_generation: AtomicU64::new(0),
            failed_unlocks: AtomicU32::new(0),
            retry_after: Mutex::new(None),
        }
    }

    pub fn with_account<T>(&self, f: impl FnOnce(&mut Account) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.account.lock().map_err(|_| AppError::internal("estado corrompido"))?;
        match guard.as_mut() {
            Some(acct) => f(acct),
            None => Err(self.open_error_clone()),
        }
    }

    fn open_error_clone(&self) -> AppError {
        match &self.open_error {
            Some(e) => AppError::new(e.code, e.message.clone()),
            None => AppError::internal("cofre indisponível"),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().map(|s| s.clone()).unwrap_or_default()
    }

    // ---- Secret Key on this device (DPAPI) ---------------------------------------------

    fn secret_key_path(&self) -> PathBuf {
        self.dir.join(SECRET_KEY_FILE)
    }

    pub fn store_secret_key(&self, account_id: Uuid, key: &SecretKey) -> AppResult<()> {
        let mut entropy = DPAPI_ENTROPY.to_vec();
        entropy.extend_from_slice(account_id.as_bytes());
        let blob = platform::protect(key.as_bytes(), &entropy)
            .map_err(|e| AppError::new("keystore", format!("Não conseguimos guardar a Chave Secreta no Windows: {e}")))?;
        let stored = StoredSecretKey { account_id, blob };
        let tmp = self.dir.join(format!("{SECRET_KEY_FILE}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec(&stored).expect("serializes"))?;
        std::fs::rename(tmp, self.secret_key_path())?;
        Ok(())
    }

    /// The Secret Key stored on this device for `account_id`, if readable.
    pub fn load_secret_key(&self, account_id: Uuid) -> Option<SecretKey> {
        let raw = std::fs::read(self.secret_key_path()).ok()?;
        let stored: StoredSecretKey = serde_json::from_slice(&raw).ok()?;
        if stored.account_id != account_id {
            return None;
        }
        let mut entropy = DPAPI_ENTROPY.to_vec();
        entropy.extend_from_slice(account_id.as_bytes());
        let bytes: Zeroizing<Vec<u8>> = platform::unprotect(&stored.blob, &entropy).ok()?;
        SecretKey::from_bytes(&bytes)
    }

    pub fn has_secret_key(&self, account_id: Uuid) -> bool {
        self.load_secret_key(account_id).is_some()
    }

    pub fn hwnd(&self) -> isize {
        self.main_hwnd.load(Ordering::Relaxed)
    }
}
