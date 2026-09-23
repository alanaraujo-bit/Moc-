//! Mocó core — shared by every Mocó client (desktop today; extension, Android and iOS
//! later). Owns all cryptography and the vault model. UI layers never see raw keys.

pub mod account;
pub mod crypto;
pub mod error;
pub mod model;
pub mod store;
pub(crate) mod util;

pub use error::{CoreError, Result};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
