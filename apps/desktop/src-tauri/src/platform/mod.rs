//! OS integration. Windows is the shipping target; other platforms get safe stubs so the
//! crate keeps compiling (and the core stays testable) elsewhere.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(windows))]
mod fallback;
#[cfg(not(windows))]
pub use self::fallback::*;
