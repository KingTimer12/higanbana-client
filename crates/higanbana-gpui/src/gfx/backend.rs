//! Backends de renderização e enumeração dos disponíveis.
use higanbana_gpui_sys as sys;
use std::ffi::CStr;

/// Backend de GPU. `Auto` deixa o core escolher (D3D11 -> OpenGL -> WGPU).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BackendKind {
    #[default]
    Auto,
    D3D11,
    D3D12,
    Vulkan,
    OpenGL,
    Wgpu,
}

/// Backend disponível nesta máquina, com o nome do adaptador (GPU).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendInfo {
    pub kind: BackendKind,
    pub name: String,
    pub adapter: String,
}

impl BackendKind {
    pub(crate) fn to_raw(self) -> sys::hg_backend_kind {
        match self {
            Self::Auto => sys::hg_backend_kind_HG_BACKEND_AUTO,
            Self::D3D11 => sys::hg_backend_kind_HG_BACKEND_D3D11,
            Self::D3D12 => sys::hg_backend_kind_HG_BACKEND_D3D12,
            Self::Vulkan => sys::hg_backend_kind_HG_BACKEND_VULKAN,
            Self::OpenGL => sys::hg_backend_kind_HG_BACKEND_OPENGL,
            Self::Wgpu => sys::hg_backend_kind_HG_BACKEND_WGPU,
        }
    }

    pub(crate) fn from_raw(raw: sys::hg_backend_kind) -> Self {
        match raw {
            sys::hg_backend_kind_HG_BACKEND_D3D11 => Self::D3D11,
            sys::hg_backend_kind_HG_BACKEND_D3D12 => Self::D3D12,
            sys::hg_backend_kind_HG_BACKEND_VULKAN => Self::Vulkan,
            sys::hg_backend_kind_HG_BACKEND_OPENGL => Self::OpenGL,
            sys::hg_backend_kind_HG_BACKEND_WGPU => Self::Wgpu,
            _ => Self::Auto,
        }
    }

    /// Nome curto para UI/logs.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::D3D11 => "D3D11",
            Self::D3D12 => "D3D12",
            Self::Vulkan => "Vulkan",
            Self::OpenGL => "OpenGL",
            Self::Wgpu => "WGPU",
        }
    }
}

impl BackendInfo {
    pub(crate) fn from_raw(raw: &sys::hg_backend_info) -> Self {
        let text = |buf: &[std::ffi::c_char]| {
            let bytes: &[u8] = unsafe { &*(buf as *const [std::ffi::c_char] as *const [u8]) };
            CStr::from_bytes_until_nul(bytes).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
        };
        Self { kind: BackendKind::from_raw(raw.kind), name: text(&raw.name), adapter: text(&raw.adapter) }
    }
}

/// Registra backends implementados em Rust (WGPU). Idempotente.
pub(crate) fn install_rust_backends() {
    #[cfg(feature = "wgpu")]
    sys::wgpu_backend::install();
}

/// Lista os backends que funcionam nesta máquina.
pub fn enumerate() -> Vec<BackendInfo> {
    install_rust_backends();
    let mut raw: [sys::hg_backend_info; 8] = unsafe { std::mem::zeroed() };
    let n = unsafe { sys::hg_enumerate_backends(raw.as_mut_ptr(), raw.len() as i32) }.clamp(0, 8) as usize;
    raw[..n].iter().map(BackendInfo::from_raw).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_roundtrip() {
        for k in [BackendKind::D3D11, BackendKind::D3D12, BackendKind::Vulkan, BackendKind::OpenGL, BackendKind::Wgpu] {
            assert_eq!(BackendKind::from_raw(k.to_raw()), k);
        }
    }
}
