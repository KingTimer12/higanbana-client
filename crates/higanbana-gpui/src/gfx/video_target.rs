//! Alvo de vídeo do core (só D3D11): o mpv desenha com OpenGL na thread de render
//! direto numa textura que a UI mostra. Thread da UI: `create`, `image`, `destroy`.
//! Thread de render: `gl_init`, `gl_proc`, `begin`, `end`, `gl_release`.
use super::context::Context;
use crate::error::{check, Result};
use higanbana_gpui_sys as sys;
use std::ffi::{c_void, CStr};
use std::ptr::NonNull;

/// Ponteiro do alvo. Copiável e `Send`: quem usa respeita a divisão de threads acima e
/// só chama `destroy` depois de `gl_release` (ou se o GL nunca iniciou).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoTarget(NonNull<sys::hg_video_target>);

unsafe impl Send for VideoTarget {}
unsafe impl Sync for VideoTarget {}

impl VideoTarget {
    /// Falha (indisponível) se o backend atual não é D3D11.
    pub fn create(ctx: &Context) -> Result<VideoTarget> {
        Self::create_in(ctx.raw())
    }

    /// No contexto vivo desta thread (thread da UI); `None` se não há contexto.
    pub(crate) fn create_current() -> Option<Result<VideoTarget>> {
        super::context::current_raw().map(|c| Self::create_in(c.as_ptr()))
    }

    fn create_in(ctx: *mut sys::hg_context) -> Result<VideoTarget> {
        let mut raw = std::ptr::null_mut();
        check(unsafe { sys::hg_video_target_create(ctx, &mut raw) })?;
        Ok(VideoTarget(NonNull::new(raw).expect("hg_video_target_create devolveu nulo")))
    }

    /// Contexto GL + interop, corrente na thread que chama.
    pub fn gl_init(self) -> Result<()> {
        check(unsafe { sys::hg_video_target_gl_init(self.0.as_ptr()) })
    }

    pub fn gl_proc(self, name: &CStr) -> *mut c_void {
        unsafe { sys::hg_video_target_gl_proc(self.0.as_ptr(), name.as_ptr()) }
    }

    /// Começa um quadro; devolve o FBO ligado à textura livre.
    pub fn begin(self, width: u32, height: u32) -> Result<u32> {
        let mut fbo = 0;
        check(unsafe { sys::hg_video_target_begin(self.0.as_ptr(), width, height, &mut fbo) })?;
        Ok(fbo)
    }

    /// Publica o quadro para a UI.
    pub fn end(self) -> Result<()> {
        check(unsafe { sys::hg_video_target_end(self.0.as_ptr()) })
    }

    pub fn gl_release(self) {
        unsafe { sys::hg_video_target_gl_release(self.0.as_ptr()) }
    }

    /// Imagem do último quadro (`None` antes do primeiro) e se é nova desde a última chamada.
    pub fn image(self) -> Result<(Option<NonNull<sys::hg_image>>, bool)> {
        let (mut out, mut fresh) = (std::ptr::null_mut(), 0);
        check(unsafe { sys::hg_video_target_image(self.0.as_ptr(), &mut out, &mut fresh) })?;
        Ok((NonNull::new(out), fresh != 0))
    }

    /// # Safety
    /// Na thread da UI, depois de `gl_release` na thread de render; o ponteiro não vale mais.
    pub unsafe fn destroy(self) {
        unsafe { sys::hg_video_target_destroy(self.0.as_ptr()) }
    }
}
