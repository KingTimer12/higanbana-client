//! Superfície WGPU por janela (HWND): configuração, aquisição da textura do frame,
//! limpeza e present. Lost/Outdated reconfiguram (Lost recria a superfície).

use super::backdrop::Backdrop;
use super::device::{Gpu, SURFACE_FORMAT};
use super::{fail, Res};
use crate::*;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle};
use std::ffi::c_void;
use std::num::NonZeroIsize;

pub struct Window {
    hwnd: *mut c_void,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// vsync pedido no último end_frame; aplicado no próximo begin.
    vsync: bool,
    pub frame: Option<wgpu::SurfaceTexture>,
    pub view: Option<wgpu::TextureView>,
    pub clear: wgpu::Color,
    /// O frame atual já passou pelo clear (submit usa Load em vez de Clear).
    pub cleared: bool,
    /// Fundo copiado para shaders com backdrop (vazio até o 1º uso; solto no resize).
    pub backdrop: Backdrop,
    /// O último `begin` achou a superfície ocluída (tela bloqueada etc.): nada vai à tela.
    pub occluded: bool,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
}
const GWLP_HINSTANCE: i32 = -6;

fn create_surface(gpu: &Gpu, hwnd: *mut c_void) -> Res<wgpu::Surface<'static>> {
    let Some(handle) = NonZeroIsize::new(hwnd as isize) else { return fail(HG_ERR_INVALID, c"wgpu: HWND nulo") };
    let mut win32 = Win32WindowHandle::new(handle);
    // Vulkan exige o HINSTANCE da janela (sem ele a superfície fica sem formatos).
    win32.hinstance = NonZeroIsize::new(unsafe { GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) });
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(RawDisplayHandle::Windows(WindowsDisplayHandle::new())),
        raw_window_handle: RawWindowHandle::Win32(win32),
    };
    // SAFETY: o HWND vive até o detach, que destrói a superfície antes da janela.
    match unsafe { gpu.instance.create_surface_unsafe(target) } {
        Ok(s) => Ok(s),
        Err(_) => fail(HG_ERR_DEVICE, c"wgpu: create_surface falhou"),
    }
}

fn present_mode(caps: &wgpu::SurfaceCapabilities, vsync: bool) -> wgpu::PresentMode {
    use wgpu::PresentMode::*;
    if vsync { return Fifo }
    [Immediate, Mailbox].into_iter().find(|m| caps.present_modes.contains(m)).unwrap_or(Fifo)
}

impl Window {
    pub fn new(gpu: &Gpu, hwnd: *mut c_void, w: u32, h: u32) -> Res<Self> {
        let surface = create_surface(gpu, hwnd)?;
        let caps = surface.get_capabilities(&gpu.adapter);
        if !caps.formats.contains(&SURFACE_FORMAT) {
            return fail(HG_ERR_UNAVAILABLE, c"wgpu: superfície sem Bgra8Unorm");
        }
        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) { usage |= wgpu::TextureUsages::COPY_SRC }
        let Some(mut config) = surface.get_default_config(&gpu.adapter, w.max(1), h.max(1)) else {
            return fail(HG_ERR_UNAVAILABLE, c"wgpu: superfície incompatível com o adaptador");
        };
        config.format = SURFACE_FORMAT;
        config.usage = usage;
        config.present_mode = present_mode(&caps, true);
        config.view_formats.clear();
        let win = Window {
            hwnd, surface, config, vsync: true, frame: None, view: None,
            clear: wgpu::Color::BLACK, cleared: true, backdrop: Backdrop::default(), occluded: false,
        };
        if w > 0 && h > 0 { win.surface.configure(&gpu.device, &win.config) }
        Ok(win)
    }

    pub fn resize(&mut self, gpu: &Gpu, w: u32, h: u32) -> Res {
        if w == 0 || h == 0 || (w == self.config.width && h == self.config.height) { return Ok(()) }
        self.config.width = w;
        self.config.height = h;
        self.backdrop.release();
        self.surface.configure(&gpu.device, &self.config);
        Ok(())
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    fn reconfigure(&mut self, gpu: &Gpu) {
        let caps = self.surface.get_capabilities(&gpu.adapter);
        self.config.present_mode = present_mode(&caps, self.vsync);
        self.surface.configure(&gpu.device, &self.config);
    }

    /// Adquire a textura do frame. Timeout/Occluded: frame vira no-op.
    pub fn begin(&mut self, gpu: &Gpu, w: u32, h: u32, clear: hg_color) -> Res {
        self.drop_frame();
        self.occluded = false;
        if w != self.config.width || h != self.config.height {
            self.config.width = w;
            self.config.height = h;
            self.backdrop.release();
            self.reconfigure(gpu);
        }
        for _ in 0..3 {
            use wgpu::CurrentSurfaceTexture as T;
            match self.surface.get_current_texture() {
                T::Success(t) | T::Suboptimal(t) => {
                    self.view = Some(t.texture.create_view(&Default::default()));
                    self.frame = Some(t);
                    break;
                }
                T::Outdated => self.reconfigure(gpu),
                T::Lost => {
                    self.surface = create_surface(gpu, self.hwnd)?;
                    self.reconfigure(gpu);
                }
                T::Timeout => return Ok(()),
                // Sem frame não há present para frear o laço: o core avisa o Rust.
                T::Occluded => {
                    self.occluded = true;
                    return Ok(());
                }
                T::Validation => return fail(HG_ERR_DEVICE, c"wgpu: get_current_texture inválido"),
            }
        }
        if self.frame.is_none() { return fail(HG_ERR_DEVICE, c"wgpu: superfície perdida") }
        self.clear = wgpu::Color { r: clear.r as f64, g: clear.g as f64, b: clear.b as f64, a: clear.a as f64 };
        self.cleared = false;
        Ok(())
    }

    /// Apresenta o frame (limpando antes se ninguém desenhou).
    pub fn end(&mut self, gpu: &Gpu, vsync: bool) -> Res {
        if !self.cleared {
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            drop(self.begin_pass(&mut enc));
            gpu.queue.submit([enc.finish()]);
        }
        self.view = None;
        if let Some(frame) = self.frame.take() { gpu.queue.present(frame) }
        if vsync != self.vsync {
            self.vsync = vsync;
            self.reconfigure(gpu);
        }
        Ok(())
    }

    /// Render pass no backbuffer: Clear na primeira vez do frame, Load depois.
    pub fn begin_pass<'e>(&mut self, enc: &'e mut wgpu::CommandEncoder) -> wgpu::RenderPass<'e> {
        let load = if self.cleared { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(self.clear) };
        self.cleared = true;
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: self.view.as_ref().expect("begin_pass sem frame"),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    fn drop_frame(&mut self) {
        self.view = None;
        self.frame = None;
    }
}
