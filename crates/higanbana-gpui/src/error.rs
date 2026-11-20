//! Erros do core C traduzidos para Rust.
use higanbana_gpui_sys as sys;
use std::ffi::CStr;
use std::fmt;

/// Categoria do erro devolvido pelo core (`hg_result`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Unavailable,
    Device,
    OutOfMemory,
    Invalid,
    Other(i32),
}

/// Erro com a mensagem de `hg_last_error()` capturada no momento da falha.
#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

pub type Result<T> = std::result::Result<T, Error>;

impl ErrorKind {
    fn from_code(code: i32) -> Self {
        match code as u32 {
            sys::HG_ERR_UNAVAILABLE => Self::Unavailable,
            sys::HG_ERR_DEVICE => Self::Device,
            sys::HG_ERR_OOM => Self::OutOfMemory,
            sys::HG_ERR_INVALID => Self::Invalid,
            _ => Self::Other(code),
        }
    }
}

impl Error {
    /// Erro de argumento inválido detectado do lado Rust.
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self { kind: ErrorKind::Invalid, message: message.into() }
    }
}

/// Converte um `hg_result` em `Result`, lendo a mensagem thread-local do core.
pub(crate) fn check(code: sys::hg_result) -> Result<()> {
    if code == sys::HG_OK as i32 {
        return Ok(());
    }
    let ptr = unsafe { sys::hg_last_error() };
    let message =
        if ptr.is_null() { String::new() } else { unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned() };
    Err(Error { kind: ErrorKind::from_code(code), message })
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {}
