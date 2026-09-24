mod autolock;
mod breach;
mod commands;
mod devseed;
mod dto;
mod error;
mod files;
mod hello;
mod native;
mod platform;
mod settings;
mod state;
mod transfer;
mod updates;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};

use settings::Settings;
use state::AppState;

pub(crate) fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Applies settings that affect native behavior.
pub(crate) fn apply_settings(app: &AppHandle, s: &Settings) {
    // Debug builds allow capture so development screenshots work; release honors the
    // user's choice (on by default).
    let protect = s.screen_capture_protection && !cfg!(debug_assertions);
    for (_, w) in app.webview_windows() {
        let _ = w.set_content_protected(protect);
    }
    native::apply_shortcut(app, s);
    native::apply_autostart(app, s);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir o Mocó", true, None::<&str>)?;
    let quick = MenuItem::with_id(app, "quick", "Acesso rápido", true, Some("CommandOrControl+Shift+Space"))?;
    let lock = MenuItem::with_id(app, "lock", "Trancar agora", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair do Mocó", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quick, &lock, &PredefinedMenuItem::separator(app)?, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("Mocó")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quick" => native::toggle_quick(app),
            "lock" => commands::lock_now(app, "manual"),
            "quit" => {
                commands::lock_now(app, "quit");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if !argv.iter().any(|a| a == native::HIDDEN_ARG) {
                show_main(app)
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![native::HIDDEN_ARG]),
        ))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .setup(|app| {
            let mut dir = app.path().app_local_data_dir()?;
            // Development runs can point at a throwaway profile.
            if cfg!(debug_assertions) {
                if let Ok(custom) = std::env::var("MOCO_DATA_DIR") {
                    dir = custom.into();
                }
            }
            std::fs::create_dir_all(&dir)?;
            let state = Arc::new(AppState::new(dir));
            app.manage(state.clone());

            if let Some(w) = app.get_webview_window("main") {
                #[cfg(windows)]
                if let Ok(h) = w.hwnd() {
                    state.main_hwnd.store(h.0 as isize, Ordering::Relaxed);
                }
                // Started by Windows at login: stay in the tray until asked.
                if !std::env::args().any(|a| a == native::HIDDEN_ARG) {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            apply_settings(app.handle(), &state.settings());
            build_tray(app.handle())?;
            autolock::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            let app = window.app_handle();
            let settings = app.state::<Arc<AppState>>().settings();
            match event {
                WindowEvent::CloseRequested { api, .. } if settings.close_to_tray => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::Resized(_) if settings.lock_on_minimize => {
                    if window.is_minimized().unwrap_or(false) {
                        commands::lock_now(app, "minimize");
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::account_create,
            commands::account_unlock,
            commands::account_lock,
            commands::account_unlock_hello,
            commands::hello_enable,
            commands::hello_disable,
            commands::account_recover,
            commands::account_security,
            commands::secret_key_reveal,
            commands::password_change,
            commands::recovery_rotate,
            commands::recovery_disable,
            commands::vaults_list,
            commands::vault_create,
            commands::vault_update,
            commands::vault_delete,
            commands::items_list,
            commands::item_get,
            commands::item_get_for_edit,
            commands::item_create,
            commands::item_update,
            commands::item_set_favorite,
            commands::item_set_archived,
            commands::item_trash,
            commands::item_restore,
            commands::item_purge,
            commands::trash_empty,
            commands::item_move,
            commands::item_duplicate,
            commands::item_history,
            commands::item_restore_version,
            commands::item_touch,
            commands::field_reveal,
            commands::field_copy,
            commands::copy_text,
            commands::totp_now,
            commands::totp_preview,
            commands::generator_generate,
            commands::strength_estimate,
            commands::health_report,
            commands::breach_check,
            commands::settings_get,
            commands::settings_update,
            commands::open_url,
            devseed::dev_seed,
            native::quick_hide,
            files::attachment_add,
            files::attachment_add_paths,
            files::attachment_save,
            files::attachment_preview,
            files::attachment_remove,
            files::wifi_qr,
            updates::update_check,
            updates::update_install,
            native::quick_toggle,
            native::quick_open_in_main,
            transfer::import_pick,
            transfer::import_backup_pick,
            transfer::import_commit,
            transfer::import_cancel,
            transfer::export_backup,
            transfer::export_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mocó");
}
