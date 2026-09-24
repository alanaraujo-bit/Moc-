//! Non-Windows stubs. Mocó ships on Windows first; these keep other targets compiling.
//! They are deliberately *not* insecure fallbacks: protection fails closed.

use zeroize::Zeroizing;

pub fn protect(_data: &[u8], _entropy: &[u8]) -> Result<Vec<u8>, String> {
    Err("armazenamento protegido indisponível nesta plataforma".into())
}

pub fn unprotect(_data: &[u8], _entropy: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    Err("armazenamento protegido indisponível nesta plataforma".into())
}

pub fn copy_text(_owner: isize, _text: &str, _sensitive: bool) -> Result<u32, String> {
    Err("área de transferência indisponível".into())
}

pub fn clipboard_sequence() -> u32 {
    0
}

pub fn clear_clipboard_if(_owner: isize, _sequence: u32) -> bool {
    false
}

pub fn idle_millis() -> u64 {
    0
}

pub fn session_locked() -> bool {
    false
}
