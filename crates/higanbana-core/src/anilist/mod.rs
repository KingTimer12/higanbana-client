//! Metadados do AniList (https://graphql.anilist.co), só leitura.
//!
//! - [`service::AniList`]: busca em lote + casamento com nomes de pasta + cache em disco.
//! - [`client::AniListClient`]: GraphQL com respeito ao rate limit (30–90 req/min).
//! - [`matcher`]: limpa nomes de pasta da nuvem e escolhe o melhor resultado.

pub(crate) mod cache;
mod client;
mod error;
pub mod jikan;
pub mod mal_site;
mod matcher;
mod queries;
mod service;
pub mod text;
pub mod types;

pub use client::Priority;
pub use error::{AniListError, AniListResult};
pub use queries::{MediaType, idmal_batch_query, season_at};
pub use service::{AniList, Lookup, LookupRequest};
