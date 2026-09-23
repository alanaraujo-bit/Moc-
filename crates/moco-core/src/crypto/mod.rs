//! Cryptographic building blocks. Nothing here is novel: audited primitives composed in
//! documented ways (see DECISIONS.md D-003/D-004 and docs/security.md).

pub mod aead;
pub mod identity;
pub mod kdf;
pub mod keys;
pub mod random;
pub mod secret_key;

pub use aead::{Expect, Header, Purpose};
pub use keys::{SecretString, SymmetricKey};
pub use secret_key::{RecoveryCode, SecretKey};
