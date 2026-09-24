//! Windows Hello unlock (face, fingerprint or PIN).
//!
//! A TPM-backed RSA key is created by Windows Hello (KeyCredentialManager) for this app.
//! Unlocking asks Hello to sign a fixed per-account challenge; RSA PKCS#1 v1.5 signatures
//! are deterministic, so `HKDF(signature, device salt)` yields the same 256-bit key every
//! time — the key that wraps a copy of the Account Key on this device. The private key
//! never leaves the TPM and every signature requires the user's biometric/PIN gesture.
//! Enrollment signs twice and refuses to continue if the signatures differ.

use moco_core::crypto::kdf::hkdf_key;
use moco_core::crypto::{random, SymmetricKey};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};

pub const LABEL: &str = "windows-hello";

fn challenge(account: Uuid) -> Vec<u8> {
    let mut c = b"moco/v1/hello".to_vec();
    c.extend_from_slice(account.as_bytes());
    c
}

fn derive(signature: &[u8], salt: &[u8], account: Uuid) -> SymmetricKey {
    let info = format!("moco/v1/hello-wrap/{account}");
    hkdf_key(signature, Some(salt), &info)
}

pub fn new_salt() -> [u8; 32] {
    random::bytes::<32>()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::HSTRING;
    use windows::Security::Credentials::{
        KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
    };
    use windows::Security::Cryptography::CryptographicBuffer;

    fn name(account: Uuid) -> HSTRING {
        HSTRING::from(format!("Moco-{account}"))
    }

    fn status_error(s: KeyCredentialStatus) -> AppError {
        match s {
            KeyCredentialStatus::UserCanceled => AppError::new("hello_canceled", "Windows Hello cancelado."),
            KeyCredentialStatus::NotFound => {
                AppError::new("hello_missing", "A credencial do Windows Hello não existe mais (o PIN foi redefinido?). Use a senha mestra e ative de novo.")
            }
            KeyCredentialStatus::SecurityDeviceLocked => {
                AppError::new("hello_locked", "O Windows Hello está bloqueado por muitas tentativas. Use a senha mestra.")
            }
            _ => AppError::new("hello_failed", "O Windows Hello não respondeu. Use a senha mestra."),
        }
    }

    pub fn available() -> bool {
        KeyCredentialManager::IsSupportedAsync().and_then(|op| op.get()).unwrap_or(false)
    }

    fn sign(cred: &windows::Security::Credentials::KeyCredential, data: &[u8]) -> AppResult<Zeroizing<Vec<u8>>> {
        let buf = CryptographicBuffer::CreateFromByteArray(data).map_err(|e| AppError::internal(e.message()))?;
        let res = cred.RequestSignAsync(&buf).and_then(|op| op.get()).map_err(|e| AppError::internal(e.message()))?;
        let st = res.Status().map_err(|e| AppError::internal(e.message()))?;
        if st != KeyCredentialStatus::Success {
            return Err(status_error(st));
        }
        let out = res.Result().map_err(|e| AppError::internal(e.message()))?;
        let mut arr = windows::core::Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&out, &mut arr).map_err(|e| AppError::internal(e.message()))?;
        Ok(Zeroizing::new(arr.to_vec()))
    }

    /// Creates (or replaces) the Hello credential and returns the wrapping key.
    pub fn enroll(account: Uuid, salt: &[u8]) -> AppResult<SymmetricKey> {
        if !available() {
            return Err(AppError::new("hello_unavailable", "O Windows Hello não está configurado neste computador."));
        }
        let res = KeyCredentialManager::RequestCreateAsync(&name(account), KeyCredentialCreationOption::ReplaceExisting)
            .and_then(|op| op.get())
            .map_err(|e| AppError::internal(e.message()))?;
        let st = res.Status().map_err(|e| AppError::internal(e.message()))?;
        if st != KeyCredentialStatus::Success {
            return Err(status_error(st));
        }
        let cred = res.Credential().map_err(|e| AppError::internal(e.message()))?;
        let a = sign(&cred, &challenge(account))?;
        let b = sign(&cred, &challenge(account))?;
        if *a != *b {
            let _ = KeyCredentialManager::DeleteAsync(&name(account)).and_then(|op| op.get());
            return Err(AppError::new(
                "hello_unsupported",
                "Este dispositivo do Windows Hello não gera assinaturas estáveis. Continue usando a senha mestra.",
            ));
        }
        Ok(derive(&a, salt, account))
    }

    pub fn unlock_key(account: Uuid, salt: &[u8]) -> AppResult<SymmetricKey> {
        let res = KeyCredentialManager::OpenAsync(&name(account))
            .and_then(|op| op.get())
            .map_err(|e| AppError::internal(e.message()))?;
        let st = res.Status().map_err(|e| AppError::internal(e.message()))?;
        if st != KeyCredentialStatus::Success {
            return Err(status_error(st));
        }
        let cred = res.Credential().map_err(|e| AppError::internal(e.message()))?;
        let sig = sign(&cred, &challenge(account))?;
        Ok(derive(&sig, salt, account))
    }

    pub fn remove(account: Uuid) {
        let _ = KeyCredentialManager::DeleteAsync(&name(account)).and_then(|op| op.get());
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn available() -> bool {
        false
    }
    pub fn enroll(_: Uuid, _: &[u8]) -> AppResult<SymmetricKey> {
        Err(AppError::new("hello_unavailable", "Indisponível nesta plataforma."))
    }
    pub fn unlock_key(_: Uuid, _: &[u8]) -> AppResult<SymmetricKey> {
        Err(AppError::new("hello_unavailable", "Indisponível nesta plataforma."))
    }
    pub fn remove(_: Uuid) {}
}

pub use imp::{available, enroll, remove, unlock_key};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_is_bound_to_salt_and_account() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        let sig = [7u8; 256];
        assert_eq!(derive(&sig, &[1; 32], a), derive(&sig, &[1; 32], a));
        assert_ne!(derive(&sig, &[1; 32], a), derive(&sig, &[2; 32], a));
        assert_ne!(derive(&sig, &[1; 32], a), derive(&sig, &[1; 32], b));
    }
}
