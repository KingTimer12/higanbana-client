//! Render API "opengl" do mpv desenhando no FBO do alvo de vídeo (thread de render).
use super::api::*;
use crate::gfx::VideoTarget;
use std::ffi::{c_char, c_int, c_void, CStr};

/// `get_proc_address` do mpv: resolve pelo contexto GL do alvo.
unsafe extern "C" fn get_proc(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    let target = unsafe { &*(ctx as *const VideoTarget) };
    target.gl_proc(unsafe { CStr::from_ptr(name) })
}

pub(crate) struct GlRender {
    ctx: *mut c_void,
    /// Endereço estável passado ao mpv como contexto do `get_proc`.
    target: Box<VideoTarget>,
}

impl GlRender {
    /// Liga o GL do alvo nesta thread e cria o contexto de render "opengl".
    pub fn start(api: &Api, handle: Handle, target: VideoTarget, update: (UpdateFn, *mut c_void)) -> Result<GlRender, String> {
        target.gl_init().map_err(|e| e.to_string())?;
        let target = Box::new(target);
        let mut init = OpenGlInitParams { get_proc_address: get_proc, get_proc_address_ctx: &*target as *const VideoTarget as *mut c_void };
        let mut params = [
            RenderParam { kind: RENDER_PARAM_API_TYPE, data: c"opengl".as_ptr() as *mut c_void },
            RenderParam { kind: RENDER_PARAM_OPENGL_INIT_PARAMS, data: &mut init as *mut _ as *mut c_void },
            RenderParam { kind: RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
        ];
        let mut ctx = std::ptr::null_mut();
        let code = unsafe { (api.render_create)(&mut ctx, handle, params.as_mut_ptr()) };
        if code < 0 {
            target.gl_release();
            return Err(format!("render opengl do mpv: {}", error_text(code)));
        }
        unsafe { (api.render_set_update_callback)(ctx, Some(update.0), update.1) };
        Ok(GlRender { ctx, target })
    }

    pub fn ctx(&self) -> *mut c_void {
        self.ctx
    }

    /// Desenha o quadro atual em w×h e publica para a UI.
    pub fn render(&self, api: &Api, (w, h): (u32, u32)) -> Result<(), String> {
        let fbo = self.target.begin(w, h).map_err(|e| e.to_string())?;
        let mut target = OpenGlFbo { fbo: fbo as c_int, w: w as c_int, h: h as c_int, internal_format: 0 };
        // Sem inverter: no FBO o mpv já põe o topo da imagem na linha 0, que a textura
        // D3D11 lê como topo (o teste de orientação confere).
        let mut flip: c_int = 0;
        let mut params = [
            RenderParam { kind: RENDER_PARAM_OPENGL_FBO, data: &mut target as *mut _ as *mut c_void },
            RenderParam { kind: RENDER_PARAM_FLIP_Y, data: &mut flip as *mut _ as *mut c_void },
            RenderParam { kind: RENDER_PARAM_INVALID, data: std::ptr::null_mut() },
        ];
        let code = unsafe { (api.render_render)(self.ctx, params.as_mut_ptr()) };
        let ended = self.target.end().map_err(|e| e.to_string());
        if code < 0 {
            return Err(format!("render opengl: {}", error_text(code)));
        }
        ended
    }

    /// Solta o contexto do mpv e o GL do alvo (antes de a thread sair).
    pub fn free(self, api: &Api) {
        unsafe { (api.render_free)(self.ctx) };
        self.target.gl_release();
    }
}
