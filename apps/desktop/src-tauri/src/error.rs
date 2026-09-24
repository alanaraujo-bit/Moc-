//! Errors crossing the IPC boundary. The UI matches on `code`; `message` is already
//! written for people (PT-BR). Never include secrets in either.

use moco_core::CoreError;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }
}

impl From<CoreError> for AppError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::BadCredentials => AppError::new("bad_credentials", "Senha mestra incorreta."),
            CoreError::Locked => AppError::new("locked", "Seu Mocó está trancado."),
            CoreError::Integrity => {
                AppError::new("integrity", "Alguns dados não passaram na verificação de integridade e não foram abertos.")
            }
            CoreError::Unsupported(m) => AppError::new("unsupported", m),
            CoreError::Invalid(m) => AppError::new("invalid", capitalize(&m)),
            CoreError::NotFound(m) => AppError::new("not_found", format!("Não encontramos: {m}.")),
            CoreError::Conflict(m) => AppError::new("conflict", capitalize(&m)),
            CoreError::Storage(m) => AppError::new("storage", format!("Não conseguimos gravar no disco ({m}).")),
            CoreError::Io(e) => AppError::new("io", format!("Erro de arquivo: {e}.")),
            CoreError::Serde(_) => AppError::new("integrity", "Dados em formato inesperado."),
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => {
            let mut out = f.to_uppercase().collect::<String>() + c.as_str();
            if !out.ends_with('.') && !out.ends_with('?') && !out.ends_with('!') {
                out.push('.');
            }
            out
        }
        None => String::new(),
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::internal(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("io", format!("Erro de arquivo: {e}."))
    }
}
