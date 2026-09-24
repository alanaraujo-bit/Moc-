//! Signed automatic updates (minisign signatures verified by tauri-plugin-updater before
//! anything runs). Channels map to GitHub release feeds. A release whose notes contain
//! `[critica]` is a security update and the UI treats it as such.

use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
#[cfg(desktop)]
use tauri::Emitter;
#[cfg(desktop)]
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

const REPO: &str = "https://github.com/alanaraujo-bit/Moc-";

#[cfg(desktop)]
pub type PendingUpdate = Mutex<Option<Update>>;
#[cfg(mobile)]
pub type PendingUpdate = Mutex<Option<()>>;

#[cfg(desktop)]
fn endpoint(channel: &str) -> String {
    match channel {
        "beta" => format!("{REPO}/releases/download/beta/latest.json"),
        _ => format!("{REPO}/releases/latest/download/latest.json"),
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: String,
    pub date: Option<String>,
    pub critical: bool,
}

#[cfg(desktop)]
#[tauri::command]
pub async fn update_check(app: AppHandle, state: State<'_, Arc<AppState>>) -> AppResult<Option<UpdateInfo>> {
    let channel = state.settings().update_channel;
    let url = endpoint(&channel).parse().map_err(|_| AppError::internal("endpoint inválido"))?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|e| AppError::internal(e.to_string()))?
        .build()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let update = updater.check().await.map_err(|e| {
        AppError::new("offline", format!("Não deu para procurar atualizações agora ({e})."))
    })?;
    let info = update.as_ref().map(|u| {
        let notes = u.body.clone().unwrap_or_default();
        UpdateInfo {
            version: u.version.clone(),
            current_version: u.current_version.clone(),
            critical: notes.to_lowercase().contains("[critica]") || notes.to_lowercase().contains("[crítica]"),
            notes: notes.replace("[critica]", "").replace("[crítica]", "").trim().to_string(),
            date: u.date.map(|d| d.to_string()),
        }
    });
    if let Ok(mut g) = state.pending_update.lock() {
        *g = update;
    }
    Ok(info)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg(desktop)]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

/// Downloads, verifies the signature, installs and restarts.
#[cfg(desktop)]
#[tauri::command]
pub async fn update_install(app: AppHandle, state: State<'_, Arc<AppState>>) -> AppResult<()> {
    let update = state
        .pending_update
        .lock()
        .map_err(|_| AppError::internal("estado"))?
        .take()
        .ok_or_else(|| AppError::new("no_update", "Nenhuma atualização pendente."))?;
    let mut downloaded = 0u64;
    let emitter = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = emitter.emit("moco://update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| AppError::new("update_failed", format!("A atualização não foi instalada: {e}. Nada mudou no seu Mocó.")))?;
    // Lock before restarting so keys don't linger across the swap.
    crate::commands::lock_now(&app, "update");
    app.restart();
}

// On phones updates come from the app store (or a new APK), not from the app itself.
#[cfg(mobile)]
#[tauri::command]
pub async fn update_check(_app: AppHandle, _state: State<'_, Arc<AppState>>) -> AppResult<Option<UpdateInfo>> {
    Ok(None)
}

#[cfg(mobile)]
#[tauri::command]
pub async fn update_install(_app: AppHandle, _state: State<'_, Arc<AppState>>) -> AppResult<()> {
    Err(AppError::new("no_update", "Atualize o Mocó pela loja de aplicativos."))
}
