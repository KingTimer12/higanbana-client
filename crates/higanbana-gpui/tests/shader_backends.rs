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

/// Troca em tempo de execução com vidro de backdrop na tela, passando por todos os pares de
/// backends nativos nos dois sentidos (o WGPU troca em `shader_wgpu`: não divide processo com o Vulkan).
#[test]
fn troca_de_backend_com_backdrop() {
    use BackendKind::*;
    case_backend_switch(D3D11, &[D3D12, OpenGL, Vulkan, D3D11, Vulkan, D3D12, D3D11, OpenGL, D3D12]);
}
