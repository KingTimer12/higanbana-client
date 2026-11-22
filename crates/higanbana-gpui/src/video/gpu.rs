//! Ligação entre o player, a thread de render e a UI para o vídeo na GPU (D3D11).
//!
//! O player cria o alvo no contexto da thread da UI antes de a thread de render nascer;
//! ela já começa com o mpv em OpenGL e marca `active`. Qualquer falha (interop recusada,
//! quadro que não desenha, backend trocado) marca `failed` e o render volta à CPU.
use crate::element::ExternalImage;
use crate::gfx::{Context, VideoTarget};

use higanbana_gpui_sys as sys;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, MutexGuard};

/// Por onde os quadros chegam à tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPath {
    /// mpv converte na CPU (render "sw") e o quadro sobe como imagem.
    Cpu,
    /// mpv desenha com OpenGL direto na textura D3D11 mostrada.
    Gpu,
}

#[derive(Default)]
struct Inner {
    target: Option<VideoTarget>,
    failed: bool,
    active: bool,
}

#[derive(Default)]
pub(crate) struct GpuLink(Mutex<Inner>);

impl GpuLink {
    pub fn with_target(target: VideoTarget) -> GpuLink {
        GpuLink(Mutex::new(Inner { target: Some(target), failed: false, active: false }))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Thread de render: alvo pronto para ligar o GL (ainda não ativo, sem falha).
    pub fn pending_target(&self) -> Option<VideoTarget> {
        let g = self.lock();
        if g.failed || g.active { None } else { g.target }
    }

    pub fn set_active(&self, on: bool) {
        self.lock().active = on;
    }

    pub fn fail(&self) {
        self.lock().failed = true;
    }

    pub fn failed(&self) -> bool {
        self.lock().failed
    }

    pub fn active(&self) -> bool {
        let g = self.lock();
        g.active && !g.failed
    }

    /// Player: tira o alvo para destruir (depois de a thread de render ter saído).
    pub fn take_target(&self) -> Option<VideoTarget> {
        self.lock().target.take()
    }
}

/// O lado da UI: imagem externa que mostra o último quadro do alvo.
pub(crate) struct GpuSource(pub Arc<GpuLink>);

impl ExternalImage for GpuSource {
    fn image(&self, _ctx: &Context) -> Option<NonNull<sys::hg_image>> {
        let target = {
            let g = self.0.lock();
            if g.failed || !g.active {
                return None;
            }
            g.target?
        };
        match target.image() {
            Ok((img, _)) => img,
            Err(e) => {
                eprintln!("higanbana-gpui: vídeo na GPU caiu para a CPU: {e}");
                self.0.fail();
                None
            }
        }
    }
}
