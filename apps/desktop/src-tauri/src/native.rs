//! Windows integration: the Quick Access window, the global shortcut, launch at startup,
//! and small window-chrome touches.

#[cfg(desktop)]
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
#[cfg(desktop)]
use tauri::{WebviewUrl, WebviewWindowBuilder, WindowEvent};
#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;
#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::error::{AppError, AppResult};
use crate::settings::Settings;
#[cfg(desktop)]
use crate::state::AppState;

pub const QUICK: &str = "quick";
pub const HIDDEN_ARG: &str = "--hidden";

/// Rounded corners for undecorated windows on Windows 11.
#[cfg(windows)]
pub fn round_corners(w: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
    if let Ok(h) = w.hwnd() {
        let pref = DWMWCP_ROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(h.0),
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const _,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
}

#[cfg(not(windows))]
pub fn round_corners(_: &tauri::WebviewWindow) {}

#[cfg(desktop)]
fn quick_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(QUICK) {
        return Ok(w);
    }
    let w = WebviewWindowBuilder::new(app, QUICK, WebviewUrl::App("index.html?w=quick".into()))
        .title("Mocó — acesso rápido")
        .inner_size(640.0, 440.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .visible(false)
        .focused(true)
        .shadow(true)
        .build()?;
    round_corners(&w);
    let protect = app.state::<Arc<AppState>>().settings().screen_capture_protection && !cfg!(debug_assertions);
    let _ = w.set_content_protected(protect);
    let handle = w.clone();
    w.on_window_event(move |e| {
        if let WindowEvent::Focused(false) = e {
            let _ = handle.hide();
        }
    });
    Ok(w)
}

#[cfg(desktop)]
pub fn toggle_quick(app: &AppHandle) {
    let Ok(w) = quick_window(app) else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    let _ = w.center();
    let _ = w.show();
    let _ = w.set_focus();
    let _ = w.emit("moco://quick-shown", ());
}

#[cfg(desktop)]
pub fn apply_shortcut(app: &AppHandle, s: &Settings) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if s.quick_access_shortcut.trim().is_empty() {
        return;
    }
    match s.quick_access_shortcut.parse::<Shortcut>() {
        Ok(sc) => {
            if let Err(e) = gs.on_shortcut(sc, |app, _sc, ev| {
                if ev.state() == ShortcutState::Pressed {
                    toggle_quick(app);
                }
            }) {
                eprintln!("atalho global indisponível: {e}");
            }
        }
        Err(e) => eprintln!("atalho inválido: {e}"),
    }
}

#[cfg(desktop)]
pub fn apply_autostart(app: &AppHandle, s: &Settings) {
    let al = app.autolaunch();
    let enabled = al.is_enabled().unwrap_or(false);
    if s.launch_at_startup && !enabled {
        let _ = al.enable();
    } else if !s.launch_at_startup && enabled {
        let _ = al.disable();
    }
}

#[tauri::command]
pub async fn quick_hide(app: AppHandle) -> AppResult<()> {
    if let Some(w) = app.get_webview_window(QUICK) {
        w.hide().map_err(|e| AppError::internal(e.to_string()))?;
    }
    Ok(())
}

/// Opens the main window on an item (from Quick Access).
#[tauri::command]
pub async fn quick_open_in_main(app: AppHandle, item_id: String) -> AppResult<()> {
    if let Some(w) = app.get_webview_window(QUICK) {
        let _ = w.hide();
    }
    crate::show_main(&app);
    let _ = app.emit_to("main", "moco://open-item", item_id);
    Ok(())
}

#[tauri::command]
pub async fn quick_toggle(app: AppHandle) -> AppResult<()> {
    toggle_quick(&app);
    Ok(())
}

// Phones have no Quick Access window, global shortcut or launch-at-login.
#[cfg(mobile)]
pub fn toggle_quick(_: &AppHandle) {}
#[cfg(mobile)]
pub fn apply_shortcut(_: &AppHandle, _: &Settings) {}
#[cfg(mobile)]
pub fn apply_autostart(_: &AppHandle, _: &Settings) {}
