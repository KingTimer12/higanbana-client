//! Shaders do usuário no WGPU, em processo separado do Vulkan: com os dois no mesmo processo,
//! `vkCreateSwapchainKHR` falha de forma intermitente (problema anterior aos shaders: acontece
//! mesmo com o WGPU sem shaders). Harness em `shader_harness/mod.rs`.
mod shader_harness;
use higanbana_gpui::prelude::*;
use shader_harness::*;

backend_tests! {
    wgpu => BackendKind::Wgpu,
}

/// Troca em tempo de execução com vidro de backdrop na tela: WGPU <-> D3D11/D3D12/OpenGL (sem
/// Vulkan, que fica em `shader_backends`).
#[test]
fn troca_de_backend_com_backdrop() {
    use BackendKind::*;
    case_backend_switch(Wgpu, &[D3D11, Wgpu, D3D12, Wgpu, OpenGL, Wgpu]);
}
