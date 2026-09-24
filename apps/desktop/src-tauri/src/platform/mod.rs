//! OS integration. Windows and Android are shipping targets; anything else gets safe stubs
//! so the crate keeps compiling (and the core stays testable) elsewhere.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use self::android::*;

#[cfg(not(any(windows, target_os = "android")))]
mod fallback;
#[cfg(not(any(windows, target_os = "android")))]
pub use self::fallback::*;
