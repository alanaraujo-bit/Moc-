//! Attachments and the Wi-Fi QR code. Files are read and written by Rust; the webview only
//! receives decrypted bytes when the person asks to preview an image.

use data_encoding::BASE64;
use qrcode::render::svg;
use qrcode::QrCode;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::dto::ItemView;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;

fn mime_of(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("heic") => "image/heic",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("pem" | "key" | "pub") => "application/x-pem-file",
        _ => "application/octet-stream",
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| AppError::internal(e.to_string()))?
}

fn attach_path(st: &AppState, item_id: Uuid, path: &Path) -> AppResult<ItemView> {
    let meta = std::fs::metadata(path)?;
    if meta.is_dir() {
        return Err(AppError::new("invalid", "Pastas não podem ser anexadas. Escolha um arquivo."));
    }
    if meta.len() > moco_core::account::MAX_ATTACHMENT_BYTES as u64 {
        return Err(AppError::new("too_large", "Arquivos anexados podem ter até 25 MB."));
    }
    let bytes = zeroize::Zeroizing::new(std::fs::read(path)?);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "arquivo".into());
    st.with_account(|a| Ok(ItemView::from(&a.add_attachment(item_id, &name, mime_of(path), &bytes)?)))
}

#[tauri::command]
pub async fn attachment_add(app: AppHandle, state: S<'_>, item_id: Uuid) -> AppResult<Option<ItemView>> {
    let st = state.inner().clone();
    blocking(move || {
        let picked = if cfg!(debug_assertions) && std::env::var("MOCO_QA_ATTACH").is_ok() {
            std::env::var("MOCO_QA_ATTACH").ok().map(PathBuf::from)
        } else {
            app.dialog().file().set_title("Anexar arquivo").blocking_pick_file().and_then(|f| f.into_path().ok())
        };
        match picked {
            Some(p) => attach_path(&st, item_id, &p).map(Some),
            None => Ok(None),
        }
    })
    .await
}

/// Files dropped onto an item (paths come from the window's drag-and-drop event).
#[tauri::command]
pub async fn attachment_add_paths(state: S<'_>, item_id: Uuid, paths: Vec<String>) -> AppResult<ItemView> {
    let st = state.inner().clone();
    blocking(move || {
        let mut last = None;
        for p in paths.iter().take(10) {
            last = Some(attach_path(&st, item_id, Path::new(p))?);
        }
        last.ok_or_else(|| AppError::new("invalid", "Nenhum arquivo."))
    })
    .await
}

#[tauri::command]
pub async fn attachment_save(app: AppHandle, state: S<'_>, item_id: Uuid, attachment_id: Uuid) -> AppResult<Option<String>> {
    let st = state.inner().clone();
    blocking(move || {
        let (meta, bytes) = st.with_account(|a| Ok(a.read_attachment(item_id, attachment_id)?))?;
        let target = if cfg!(debug_assertions) && std::env::var("MOCO_QA_SAVE_DIR").is_ok() {
            Some(PathBuf::from(std::env::var("MOCO_QA_SAVE_DIR").unwrap()).join(&meta.name))
        } else {
            app.dialog()
                .file()
                .set_title("Salvar anexo")
                .set_file_name(&meta.name)
                .blocking_save_file()
                .and_then(|f| f.into_path().ok())
        };
        let Some(path) = target else { return Ok(None) };
        std::fs::write(&path, &bytes[..])?;
        Ok(Some(path.to_string_lossy().to_string()))
    })
    .await
}

/// Data URL for previewing an image attachment inside the app (≤ 8 MB).
#[tauri::command]
pub async fn attachment_preview(state: S<'_>, item_id: Uuid, attachment_id: Uuid) -> AppResult<String> {
    let st = state.inner().clone();
    blocking(move || {
        let (meta, bytes) = st.with_account(|a| Ok(a.read_attachment(item_id, attachment_id)?))?;
        if !meta.mime.starts_with("image/") || meta.mime == "image/heic" || bytes.len() > 8 * 1024 * 1024 {
            return Err(AppError::new("unsupported", "Sem pré-visualização para este arquivo. Use “Salvar como…”."));
        }
        Ok(format!("data:{};base64,{}", meta.mime, BASE64.encode(&bytes)))
    })
    .await
}

#[tauri::command]
pub async fn attachment_remove(state: S<'_>, item_id: Uuid, attachment_id: Uuid) -> AppResult<ItemView> {
    state.with_account(|a| Ok(ItemView::from(&a.remove_attachment(item_id, attachment_id)?)))
}

fn wifi_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| if matches!(c, '\\' | ';' | ',' | ':' | '"') { vec!['\\', c] } else { vec![c] })
        .collect()
}

/// SVG QR code that phones scan to join the Wi-Fi network (standard WIFI: URI).
#[tauri::command]
pub async fn wifi_qr(state: S<'_>, item_id: Uuid) -> AppResult<String> {
    state.with_account(|a| {
        let item = a.item(item_id)?;
        let ssid = item.details.value("ssid").ok_or_else(|| AppError::new("invalid", "Preencha o nome da rede."))?;
        let pass = item.details.value("password").unwrap_or("");
        let sec = match item.details.value("security").unwrap_or("WPA2") {
            "Aberta" => "nopass",
            "WEP" => "WEP",
            _ => "WPA",
        };
        let payload = zeroize::Zeroizing::new(if sec == "nopass" {
            format!("WIFI:T:nopass;S:{};;", wifi_escape(ssid))
        } else {
            format!("WIFI:T:{sec};S:{};P:{};;", wifi_escape(ssid), wifi_escape(pass))
        });
        let code = QrCode::new(payload.as_bytes()).map_err(|e| AppError::internal(e.to_string()))?;
        Ok(code
            .render::<svg::Color>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(svg::Color("#111726"))
            .light_color(svg::Color("#ffffff"))
            .build())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes_special_characters() {
        assert_eq!(super::wifi_escape(r#"a;b,c:d"e\f"#), r#"a\;b\,c\:d\"e\\f"#);
    }
}
