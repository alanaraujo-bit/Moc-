//! Android: the Keystore stands in for DPAPI and the clipboard is marked sensitive. Both
//! live in the Kotlin `MocoPlugin` (gen/android/…/MocoPlugin.kt); these functions call it.
//! Idle time and session lock don't exist here — the app locks when it goes to the
//! background (see the frontend's visibility handling).

use data_encoding::BASE64;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::Wry;
use zeroize::Zeroizing;

static HANDLE: OnceLock<PluginHandle<Wry>> = OnceLock::new();

/// Registers the Kotlin side; must be added to the app builder on Android.
pub fn mobile_plugin() -> TauriPlugin<Wry> {
    Builder::new("moco")
        .setup(|_app, api| {
            let handle = api.register_android_plugin("app.moco.android", "MocoPlugin")?;
            let _ = HANDLE.set(handle);
            Ok(())
        })
        .build()
}

fn call<T: serde::de::DeserializeOwned>(cmd: &str, payload: impl Serialize) -> Result<T, String> {
    HANDLE.get().ok_or("plataforma não iniciada")?.run_mobile_plugin(cmd, payload).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct Seal<'a> {
    data: &'a str,
    aad: &'a str,
}

#[derive(Serialize)]
struct Open<'a> {
    blob: &'a str,
    aad: &'a str,
}

#[derive(Deserialize)]
struct Blob {
    blob: String,
}

#[derive(Deserialize)]
struct Data {
    data: String,
}

pub fn protect(data: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    let data = Zeroizing::new(BASE64.encode(data));
    let r: Blob = call("protect", Seal { data: &data, aad: &BASE64.encode(entropy) })?;
    BASE64.decode(r.blob.as_bytes()).map_err(|e| e.to_string())
}

pub fn unprotect(blob: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    let r: Data = call("unprotect", Open { blob: &BASE64.encode(blob), aad: &BASE64.encode(entropy) })?;
    let encoded = Zeroizing::new(r.data);
    BASE64.decode(encoded.as_bytes()).map(Zeroizing::new).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct Copy<'a> {
    text: &'a str,
    sensitive: bool,
}

#[derive(Deserialize)]
struct Seq {
    seq: u32,
}

#[derive(Serialize)]
struct ClearIf {
    seq: u32,
}

#[derive(Deserialize)]
struct Cleared {
    cleared: bool,
}

pub fn copy_text(_owner: isize, text: &str, sensitive: bool) -> Result<u32, String> {
    Ok(call::<Seq>("copy", Copy { text, sensitive })?.seq)
}

/// Android doesn't expose a clipboard sequence number; the plugin keeps its own.
pub fn clipboard_sequence() -> u32 {
    0
}

pub fn clear_clipboard_if(_owner: isize, sequence: u32) -> bool {
    call::<Cleared>("clearIf", ClearIf { seq: sequence }).map(|c| c.cleared).unwrap_or(false)
}

pub fn idle_millis() -> u64 {
    0
}

pub fn session_locked() -> bool {
    false
}

#[derive(Deserialize)]
struct Empty {}

/// Sends the app to the background (Android back at the top level).
pub fn move_to_background() {
    let _ = call::<Empty>("background", ());
}

#[derive(Deserialize, Serialize)]
pub struct Insets {
    pub top: f64,
    pub bottom: f64,
}

pub fn insets() -> Option<Insets> {
    call::<Insets>("insets", ()).ok()
}
