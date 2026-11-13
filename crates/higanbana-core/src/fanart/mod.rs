//! fanart.tv: capas de anime (pôsteres). Só leitura.
//!
//! Opcional: sem `FANART_TV` no build o cliente responde `NotConfigured` e as
//! capas continuam vindo do AniList.

pub mod client;
pub mod credentials;
pub mod encode;
pub mod mapping;
pub mod pick;
pub mod service;
pub mod store;

pub use service::Fanart;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FanartError {
    #[error("fanart.tv não configurado")]
    NotConfigured,
    #[error("não encontrado no fanart.tv")]
    NotFound,
    #[error("falha HTTP no fanart.tv: {0}")]
    Http(String),
    #[error("resposta inválida do fanart.tv: {0}")]
    Decode(String),
    #[error("imagem inválida: {0}")]
    Image(String),
}

pub type FanartResult<T> = Result<T, FanartError>;
