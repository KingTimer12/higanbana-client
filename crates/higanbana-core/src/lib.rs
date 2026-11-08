//! Núcleo do higanbana, sem interface: sessão, nuvem, AniList, leitor de mangá,
//! proxy de vídeo e motor de música.
//!
//! A interface cria um [`Core`] e chama os serviços dele. O que muda sozinho
//! (sessão, progresso de download, estado da música) chega como [`Event`].
//! A sessão nunca sai daqui: a interface só vê caminhos, bytes e estados.

pub mod anilist;
pub mod anime_index;
pub mod auth;
pub mod catalog;
pub mod catalog_filter;
mod cloud;
mod core;
pub mod events;
pub mod http;
pub mod library;
pub mod music;
pub mod nuvem;
pub mod paths;
pub mod player;
pub mod prefs;
pub mod reader;

pub use cloud::Cloud;
pub use core::Core;
pub use events::{Event, EventFn};
pub use paths::Paths;
