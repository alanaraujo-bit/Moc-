//! Exports.
//!
//! - **Mocó backup** (`.moco`): everything, encrypted with a password chosen for the
//!   export (Argon2id → XChaCha20-Poly1305 envelope). Portable between computers; does
//!   not depend on the account keys, so it survives losing this installation.
//! - **CSV**: logins in the de-facto browser format, in plaintext, only after an explicit
//!   warning in the UI. Never trap the user.
//!
//! Layout of `.moco`: `MAGIC (8) || u32 BE params_len || params JSON || envelope`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::aead::{self, Expect, Header, Purpose};
use crate::crypto::kdf::{self, KdfParams};
use crate::crypto::SymmetricKey;
use crate::error::{CoreError, Result};
use crate::model::{now_ms, Item, Timestamp};

pub const MAGIC: &[u8; 8] = b"MOCOEXP1";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultExport {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub format: u32,
    pub app_version: String,
    pub exported_at: Timestamp,
    pub vaults: Vec<VaultExport>,
    pub items: Vec<Item>,
}

fn export_key(password: &str, params: &KdfParams) -> Result<SymmetricKey> {
    let raw = kdf::derive_password_key(password, params)?;
    Ok(SymmetricKey::from_bytes(*raw))
}

pub fn seal_backup(backup: &Backup, password: &str) -> Result<Vec<u8>> {
    if password.chars().count() < 8 {
        return Err(CoreError::Invalid("use uma senha de exportação com pelo menos 8 caracteres".into()));
    }
    let params = KdfParams::recommended();
    let key = export_key(password, &params)?;
    let json = Zeroizing::new(serde_json::to_vec(backup)?);
    let header = Header::new(Purpose::Export, Uuid::nil()).padded();
    let env = aead::seal(&key, &header, &json);
    let p = serde_json::to_vec(&params)?;
    let mut out = Vec::with_capacity(12 + p.len() + env.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(p.len() as u32).to_be_bytes());
    out.extend_from_slice(&p);
    out.extend_from_slice(&env);
    Ok(out)
}

pub fn open_backup(data: &[u8], password: &str) -> Result<Backup> {
    if data.len() < 12 || &data[..8] != MAGIC {
        return Err(CoreError::Invalid("não é um backup do Mocó".into()));
    }
    let plen = u32::from_be_bytes(data[8..12].try_into().expect("4 bytes")) as usize;
    if data.len() < 12 + plen {
        return Err(CoreError::Integrity);
    }
    let params: KdfParams = serde_json::from_slice(&data[12..12 + plen])?;
    let key = export_key(password, &params)?;
    let (_, pt) = aead::open(&key, &data[12 + plen..], &Expect::purpose(Purpose::Export)).map_err(|e| match e {
        CoreError::Integrity => CoreError::BadCredentials,
        other => other,
    })?;
    let b: Backup = serde_json::from_slice(&pt)?;
    if b.format != 1 {
        return Err(CoreError::Unsupported(format!("backup formato {}", b.format)));
    }
    Ok(b)
}

fn csv_escape(v: &str) -> String {
    // Neutralize spreadsheet formula injection in exported cells.
    let v = if v.starts_with(['=', '+', '-', '@']) { format!("'{v}") } else { v.to_string() };
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// Browser-style CSV of logins (name,url,username,password,note).
pub fn logins_csv(items: &[Item]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::from("name,url,username,password,note\n"));
    for i in items.iter().filter(|i| i.overview.trashed_at.is_none()) {
        let pass = i.details.value("password").unwrap_or("");
        let user = i.details.value("username").unwrap_or("");
        if pass.is_empty() && user.is_empty() {
            continue;
        }
        let url = i.overview.urls.first().map(String::as_str).unwrap_or("");
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_escape(&i.overview.title),
            csv_escape(url),
            csv_escape(user),
            csv_escape(pass),
            csv_escape(i.details.notes.expose())
        ));
    }
    out
}

pub fn new_backup(app_version: &str, vaults: Vec<VaultExport>, items: Vec<Item>) -> Backup {
    Backup { format: 1, app_version: app_version.into(), exported_at: now_ms(), vaults, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretString;
    use crate::model::{Details, Field, FieldType, ItemKind, Overview};

    fn item() -> Item {
        Item {
            id: Uuid::new_v4(),
            vault_id: Uuid::nil(),
            kind: ItemKind::Login,
            overview: Overview { title: "=HYPERLINK(\"x\")".into(), urls: vec!["https://a.com".into()], ..Default::default() },
            details: Details {
                fields: vec![Field { id: "password".into(), label: String::new(), kind: FieldType::Concealed, value: SecretString::new("p,\"q"), section: None }],
                ..Default::default()
            },
            created_at: 1,
            updated_at: 1,
            revision: 1,
        }
    }

    #[test]
    fn backup_roundtrip_and_wrong_password() {
        let b = new_backup("0.1.0", vec![], vec![item()]);
        let sealed = seal_backup(&b, "senha de exportação").unwrap();
        assert!(!String::from_utf8_lossy(&sealed).contains("HYPERLINK"));
        let back = open_backup(&sealed, "senha de exportação").unwrap();
        assert_eq!(back.items.len(), 1);
        assert!(matches!(open_backup(&sealed, "outra senha qualquer"), Err(CoreError::BadCredentials)));
    }

    #[test]
    fn csv_is_escaped_and_formula_safe() {
        let csv = logins_csv(&[item()]);
        assert!(csv.contains("\"'=HYPERLINK(\"\"x\"\")\""));
        assert!(csv.contains("\"p,\"\"q\""));
    }
}
