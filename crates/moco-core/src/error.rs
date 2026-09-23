use thiserror::Error;

/// Errors surfaced by the core. Variants are deliberately coarse where finer detail
/// would leak information (e.g. we never say *which* of password/Secret Key was wrong).
#[derive(Debug, Error)]
pub enum CoreError {
    /// The master password, Secret Key or recovery code did not unlock the vault.
    #[error("credenciais incorretas")]
    BadCredentials,

    /// Authenticated decryption failed on data that should have been valid.
    #[error("dados corrompidos ou adulterados")]
    Integrity,

    /// A format/version produced by a newer (or unknown) Mocó.
    #[error("formato não suportado: {0}")]
    Unsupported(String),

    #[error("entrada inválida: {0}")]
    Invalid(String),

    #[error("o cofre está trancado")]
    Locked,

    #[error("não encontrado: {0}")]
    NotFound(String),

    #[error("já existe: {0}")]
    Conflict(String),

    #[error("erro de armazenamento: {0}")]
    Storage(String),

    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("erro de serialização: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
