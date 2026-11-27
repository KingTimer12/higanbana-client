//! Shaders do usuário no WGPU, em processo separado do Vulkan: com os dois no mesmo processo,
//! `vkCreateSwapchainKHR` falha de forma intermitente (problema anterior aos shaders: acontece
//! mesmo com o WGPU sem shaders). Harness em `shader_harness/mod.rs`.
mod shader_harness;
use higanbana_gpui::prelude::*;
use shader_harness::*;

backend_tests! {
    wgpu => BackendKind::Wgpu,
}
