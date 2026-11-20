//! Atlas R8 de glyphs (criado sob demanda) e upload das instâncias coletadas pelo
//! `hg_glyph_frame` do core.

use super::buffers::GrowBuffer;
use super::pipeline::{Pipelines, GLYPH_STRIDE};
use super::{fail, Res};
use crate::*;
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct Atlas {
    texture: wgpu::Texture,
    pub group: wgpu::BindGroup,
}

pub struct Glyphs {
    frame: *mut hg_glyph_frame,
    pub atlas: Option<Atlas>,
    pub instances: GrowBuffer,
    /// O último `prepare` produziu instâncias (senão as contagens do C são velhas).
    has_text: bool,
}

impl Drop for Glyphs {
    fn drop(&mut self) {
        unsafe { hg__glyph_frame_delete(self.frame) };
    }
}

/// Contexto do callback de upload: fila + textura do atlas.
struct Upload<'a> {
    queue: &'a wgpu::Queue,
    texture: &'a wgpu::Texture,
}

/// Escreve um retângulo R8 (`pitch` bytes por linha) no atlas.
unsafe extern "C" fn upload_r8(user: *mut c_void, at: hg_rect, r8: *const u8, pitch: u32) -> hg_result {
    let ok = catch_unwind(AssertUnwindSafe(|| {
        let up = unsafe { &*(user as *const Upload) };
        let (w, h) = (at.w as u32, at.h as u32);
        if w == 0 || h == 0 { return }
        let data = unsafe { std::slice::from_raw_parts(r8, (pitch * (h - 1) + w) as usize) };
        up.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: up.texture, mip_level: 0,
                origin: wgpu::Origin3d { x: at.x as u32, y: at.y as u32, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(pitch), rows_per_image: None },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }));
    match ok {
        Ok(()) => HG_OK as hg_result,
        Err(_) => unsafe { hg__fail(HG_ERR_DEVICE as hg_result, c"wgpu: upload do atlas falhou".as_ptr()) },
    }
}

fn create_atlas(device: &wgpu::Device, pipes: &Pipelines) -> Atlas {
    let size = unsafe { hg__glyph_atlas_size() };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("glyph atlas"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None, layout: &pipes.atlas_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&pipes.sampler) },
        ],
    });
    Atlas { texture, group }
}

impl Glyphs {
    pub fn new() -> Res<Self> {
        let frame = unsafe { hg__glyph_frame_new() };
        if frame.is_null() { return fail(HG_ERR_OOM, c"wgpu: sem memória") }
        Ok(Glyphs { frame, atlas: None, instances: GrowBuffer::new("glyphs"), has_text: false })
    }

    pub fn gpu_bytes(&self) -> u64 {
        let side = unsafe { hg__glyph_atlas_size() } as u64;
        self.instances.capacity() + if self.atlas.is_some() { side * side } else { 0 }
    }

    /// Glyphs do lote `i` no frame atual.
    pub fn batch_count(&self, i: u32) -> u32 {
        if !self.has_text { return 0 }
        unsafe { hg__glyph_frame_batch_count(self.frame, i) }
    }

    /// Rasteriza o texto da draw list, sobe bitmaps novos no atlas e as instâncias.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, pipes: &Pipelines,
                   ctx: *mut hg_context, dl: *const hg_draw_list) -> Res {
        self.has_text = false;
        if unsafe { hg__draw_list_has_text(dl) } == 0 { return Ok(()) }
        let atlas = self.atlas.get_or_insert_with(|| create_atlas(device, pipes));
        let up = Upload { queue, texture: &atlas.texture };
        let r = unsafe { hg__glyph_frame_collect(self.frame, ctx, dl, Some(upload_r8), &up as *const _ as *mut c_void) };
        if r != HG_OK as hg_result { return Err(super::Fail(r, c"wgpu: coleta de glyphs falhou")) }
        let mut count = 0;
        let data = unsafe { hg__glyph_frame_instances(self.frame, &mut count) };
        if count > 0 {
            let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, count as usize * GLYPH_STRIDE as usize) };
            self.instances.write(device, queue, bytes);
        }
        self.has_text = count > 0;
        Ok(())
    }
}
