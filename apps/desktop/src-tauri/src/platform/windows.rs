//! Windows specifics: DPAPI, protected clipboard, idle and session-lock detection.

use std::time::Duration;
use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, LocalFree, HANDLE, HLOCAL, HWND};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::StationsAndDesktops::{CloseDesktop, OpenInputDesktop, SwitchDesktop, DESKTOP_SWITCHDESKTOP, DESKTOP_CONTROL_FLAGS};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use zeroize::{Zeroize, Zeroizing};

// ---- DPAPI ------------------------------------------------------------------------

fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
}

/// Encrypts `data` for the current Windows user (DPAPI, user scope).
pub fn protect(data: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    let input = blob(data);
    let ent = blob(entropy);
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(&input, w!("Mocó"), Some(&ent), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            .map_err(|e| e.message())?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        Ok(v)
    }
}

pub fn unprotect(data: &[u8], entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    let input = blob(data);
    let ent = blob(entropy);
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&input, None, Some(&ent), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            .map_err(|e| e.message())?;
        let slice = std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize);
        let v = Zeroizing::new(slice.to_vec());
        slice.zeroize();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        Ok(v)
    }
}

// ---- Clipboard --------------------------------------------------------------------

struct ClipboardGuard;

impl ClipboardGuard {
    fn open(owner: HWND) -> Result<Self, String> {
        // Another app may hold the clipboard for a moment; retry briefly.
        for _ in 0..25 {
            if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
                return Ok(Self);
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        Err("a área de transferência está ocupada por outro programa".into())
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

unsafe fn put_bytes(format: u32, bytes: &[u8]) -> Result<(), String> {
    let size = bytes.len().max(1);
    let h = GlobalAlloc(GMEM_MOVEABLE, size).map_err(|e| e.message())?;
    let p = GlobalLock(h) as *mut u8;
    if p.is_null() {
        let _ = GlobalFree(Some(h));
        return Err("falha ao reservar memória".into());
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
    let _ = GlobalUnlock(h);
    if SetClipboardData(format, Some(HANDLE(h.0))).is_err() {
        let _ = GlobalFree(Some(h));
        return Err("falha ao copiar".into());
    }
    Ok(())
}

/// Copies `text`. When `sensitive`, marks it so Windows keeps it out of clipboard
/// history (Win+V), cloud clipboard and clipboard monitors. Returns the clipboard
/// sequence number right after our write, used to clear it later only if unchanged.
pub fn copy_text(owner: isize, text: &str, sensitive: bool) -> Result<u32, String> {
    let hwnd = HWND(owner as *mut _);
    let _guard = ClipboardGuard::open(hwnd)?;
    unsafe {
        EmptyClipboard().map_err(|e| e.message())?;
        let mut wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
        let result = put_bytes(CF_UNICODETEXT.0 as u32, bytes);
        wide.zeroize();
        result?;
        if sensitive {
            let zero = 0u32.to_le_bytes();
            let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
            let cloud = RegisterClipboardFormatW(w!("CanUploadToCloudClipboard"));
            if exclude != 0 {
                put_bytes(exclude, &[0])?;
            }
            if history != 0 {
                put_bytes(history, &zero)?;
            }
            if cloud != 0 {
                put_bytes(cloud, &zero)?;
            }
        }
    }
    drop(_guard);
    Ok(unsafe { GetClipboardSequenceNumber() })
}

pub fn clipboard_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// Clears the clipboard only if nobody wrote to it since `sequence`.
pub fn clear_clipboard_if(owner: isize, sequence: u32) -> bool {
    if clipboard_sequence() != sequence {
        return false;
    }
    let Ok(_guard) = ClipboardGuard::open(HWND(owner as *mut _)) else { return false };
    unsafe { EmptyClipboard().is_ok() }
}

// ---- Idle / session ---------------------------------------------------------------

/// Milliseconds since the last keyboard/mouse input anywhere in the session.
pub fn idle_millis() -> u64 {
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    unsafe {
        if GetLastInputInfo(&mut info).as_bool() {
            GetTickCount().wrapping_sub(info.dwTime) as u64
        } else {
            0
        }
    }
}

/// True while the workstation is locked (or the secure desktop is up).
pub fn session_locked() -> bool {
    unsafe {
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
            Ok(desk) => {
                let switched = SwitchDesktop(desk).is_ok();
                let _ = CloseDesktop(desk);
                !switched
            }
            Err(_) => true,
        }
    }
}

