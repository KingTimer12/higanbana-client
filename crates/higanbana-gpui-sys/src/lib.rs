//! Bindings crus (gerados pelo bindgen) do core C `hgui.h`.
#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Backend WGPU em Rust; registrar com `wgpu_backend::install()` antes do primeiro uso do core.
#[cfg(feature = "wgpu")]
pub mod wgpu_backend;
