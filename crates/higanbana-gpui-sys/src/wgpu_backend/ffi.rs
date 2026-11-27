//! Callbacks `extern "C"` da vtable: convertem ponteiros do core em `Gpu`/`Window`
//! e delegam aos módulos. Todo callback passa por `guard` (sem pânico no FFI).

use super::device::Gpu;
use super::surface::Window;
use super::{fail, guard, images, readback, render, user_shader, Res};
use crate::*;
use std::os::raw::c_int;

unsafe fn gpu_of<'a>(ctx: *mut hg_context) -> Res<&'a mut Gpu> {
    let p = unsafe { hg__ctx_backend_data(ctx) } as *mut Gpu;
    if p.is_null() { return fail(HG_ERR_INVALID, c"wgpu: contexto sem device") }
    Ok(unsafe { &mut *p })
}

unsafe fn window_of<'a>(win: *mut hg_window) -> Res<(&'a mut Gpu, &'a mut Window)> {
    let gpu = unsafe { gpu_of(hg__window_ctx(win))? };
    let p = unsafe { hg__window_surface(win) } as *mut Window;
    if p.is_null() { return fail(HG_ERR_INVALID, c"wgpu: janela sem superfície") }
    Ok((gpu, unsafe { &mut *p }))
}

fn window_size(win: *mut hg_window) -> (u32, u32) {
    let (mut w, mut h) = (0, 0);
    unsafe { hg__window_size(win, &mut w, &mut h) };
    (w, h)
}

pub unsafe extern "C" fn probe(info: *mut hg_backend_info) -> c_int {
    let found = std::panic::catch_unwind(|| Gpu::probe(unsafe { info.as_mut() })).unwrap_or(false);
    found as c_int
}

/// Checagem barata para a enumeração: não cria `wgpu::Instance` (que sobe os
/// drivers DX12/Vulkan/GL e fica viva). Basta um runtime DX12 ou Vulkan e GPU de hardware.
pub unsafe extern "C" fn detect(info: *mut hg_backend_info) -> c_int {
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let runtime = unsafe { hg__detect_dll(wide("d3d12.dll").as_ptr()) != 0 || hg__detect_dll(wide("vulkan-1.dll").as_ptr()) != 0 };
    (runtime && unsafe { hg__detect_adapter(info) } != 0) as c_int
}

pub unsafe extern "C" fn create(ctx: *mut hg_context) -> hg_result {
    guard(|| {
        let gpu = Gpu::new(unsafe { &mut *hg__ctx_info(ctx) })?;
        unsafe { hg__ctx_set_backend_data(ctx, Box::into_raw(Box::new(gpu)).cast()) };
        Ok(())
    })
}

pub unsafe extern "C" fn destroy(ctx: *mut hg_context) {
    guard(|| {
        let p = unsafe { hg__ctx_backend_data(ctx) } as *mut Gpu;
        unsafe { hg__ctx_set_backend_data(ctx, std::ptr::null_mut()) };
        if !p.is_null() { drop(unsafe { Box::from_raw(p) }) }
        Ok(())
    });
}

pub unsafe extern "C" fn attach(ctx: *mut hg_context, win: *mut hg_window) -> hg_result {
    guard(|| {
        let gpu = unsafe { gpu_of(ctx)? };
        let (w, h) = window_size(win);
        let surface = Window::new(gpu, unsafe { hg__window_hwnd(win) }, w, h)?;
        unsafe { hg__window_set_surface(win, Box::into_raw(Box::new(surface)).cast()) };
        Ok(())
    })
}

pub unsafe extern "C" fn detach(_ctx: *mut hg_context, win: *mut hg_window) {
    guard(|| {
        let p = unsafe { hg__window_surface(win) } as *mut Window;
        unsafe { hg__window_set_surface(win, std::ptr::null_mut()) };
        if !p.is_null() { drop(unsafe { Box::from_raw(p) }) }
        Ok(())
    });
}

pub unsafe extern "C" fn resize(win: *mut hg_window, w: u32, h: u32) -> hg_result {
    guard(|| {
        if w == 0 || h == 0 { return Ok(()) }
        let (gpu, surface) = unsafe { window_of(win)? };
        surface.resize(gpu, w, h)
    })
}

pub unsafe extern "C" fn begin_frame(win: *mut hg_window, clear: hg_color) -> hg_result {
    guard(|| {
        let (gpu, surface) = unsafe { window_of(win)? };
        let (w, h) = window_size(win);
        surface.begin(gpu, w, h, clear)?;
        if surface.occluded { unsafe { hg__window_set_occluded(win) } }
        Ok(())
    })
}

pub unsafe extern "C" fn submit(win: *mut hg_window, dl: *const hg_draw_list) -> hg_result {
    guard(|| {
        let ctx = unsafe { hg__window_ctx(win) };
        let (gpu, surface) = unsafe { window_of(win)? };
        render::draw_list(gpu, ctx, surface, dl)
    })
}

pub unsafe extern "C" fn end_frame(win: *mut hg_window, vsync: c_int) -> hg_result {
    guard(|| {
        let (gpu, surface) = unsafe { window_of(win)? };
        surface.end(gpu, vsync != 0)
    })
}

/// Cria a textura de `img` no device do contexto (chamado pelo core, sob demanda).
pub unsafe extern "C" fn texture_create(ctx: *mut hg_context, img: *mut hg_image) -> hg_result {
    guard(|| {
        let gpu = unsafe { gpu_of(ctx)? };
        let bytes = images::create(&gpu.device, &gpu.queue, &gpu.pipes, img);
        gpu.images.texture_bytes += bytes;
        Ok(())
    })
}

/// Atualiza a textura de `img` no lugar com os pixels novos do core.
pub unsafe extern "C" fn texture_update(ctx: *mut hg_context, img: *mut hg_image) -> hg_result {
    guard(|| {
        let gpu = unsafe { gpu_of(ctx)? };
        images::update(&gpu.queue, img);
        Ok(())
    })
}

/// Solta a textura de `img` (troca de backend, destroy do contexto ou da imagem).
pub unsafe extern "C" fn texture_destroy(ctx: *mut hg_context, img: *mut hg_image) {
    guard(|| {
        let bytes = images::destroy(img);
        if let Ok(gpu) = unsafe { gpu_of(ctx) } {
            gpu.images.texture_bytes = gpu.images.texture_bytes.saturating_sub(bytes);
        }
        Ok(())
    });
}

pub unsafe extern "C" fn read_pixel(win: *mut hg_window, x: u32, y: u32, out: *mut u32) -> hg_result {
    guard(|| {
        let (gpu, surface) = unsafe { window_of(win)? };
        let px = readback::pixel(gpu, surface, x, y)?;
        unsafe { *out = px };
        Ok(())
    })
}

/// Compila o shader do usuário (1º uso). Erro do wgpu fica em `sh->error` e volta como código.
pub unsafe extern "C" fn shader_prepare(ctx: *mut hg_context, sh: *mut hg_shader) -> hg_result {
    let mut code = HG_OK as hg_result;
    let r = guard(|| {
        code = user_shader::prepare(unsafe { gpu_of(ctx)? }, sh)?;
        Ok(())
    });
    if r == HG_OK as hg_result { return code }
    // Pânico ou falha fora dos escopos de erro: grava sh->error mesmo assim, senão o core
    // chamaria shader_prepare de novo a cada quadro.
    if unsafe { hg__shader_gpu_data(sh) }.is_null() {
        let msg = unsafe { std::ffi::CStr::from_ptr(hg_last_error()) }.to_string_lossy().into_owned();
        return user_shader::set_error(sh, &msg);
    }
    r
}

pub unsafe extern "C" fn shader_release(_ctx: *mut hg_context, sh: *mut hg_shader) {
    guard(|| {
        user_shader::release(sh);
        Ok(())
    });
}
