//! Shaders do usuário nos backends nativos (D3D11/D3D12/OpenGL/Vulkan). O WGPU roda em
//! `shader_wgpu` (processo próprio); o harness está em `shader_harness/mod.rs`.
mod shader_harness;
use higanbana_gpui::prelude::*;
use shader_harness::*;

backend_tests! {
    d3d11 => BackendKind::D3D11,
    d3d12 => BackendKind::D3D12,
    opengl => BackendKind::OpenGL,
    vulkan => BackendKind::Vulkan,
}
