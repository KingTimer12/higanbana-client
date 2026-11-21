//! Imagens: textura RGBA8 por `hg_image` (guardada em `img->gpu` como `Box` cru),
//! upload das instâncias `hg_image_draw` e desenho agrupado por textura.

use super::buffers::GrowBuffer;
use super::pipeline::Pipelines;
use super::{Fail, Res};
use crate::*;

/// Textura de uma imagem; vive atrás de `img->gpu` até `texture_destroy`.
pub struct Texture {
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    bytes: u64,
}

fn image_size(img: *const hg_image) -> (u32, u32) {
    let (mut w, mut h) = (0, 0);
    unsafe { hg_image_size(img, &mut w, &mut h) };
    (w, h)
}

/// Cria a textura de `img` a partir dos pixels do core e a pendura em `img->gpu`.
pub fn create(device: &wgpu::Device, queue: &wgpu::Queue, pipes: &Pipelines, img: *mut hg_image) -> u64 {
    let (w, h) = image_size(img);
    let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("image"), size, mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let bytes = w as u64 * h as u64 * 4;
    // SAFETY: o core garante `width*height*4` bytes compactos em `pixels`.
    let pixels = unsafe { std::slice::from_raw_parts(hg__image_pixels(img), bytes as usize) };
    queue.write_texture(
        texture.as_image_copy(), pixels,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: None },
        size,
    );
    let view = texture.create_view(&Default::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None, layout: &pipes.atlas_layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&pipes.linear) },
        ],
    });
    let tex = Box::new(Texture { texture, group, bytes });
    unsafe { hg__image_set_gpu_data(img, Box::into_raw(tex).cast()) };
    bytes
}

/// Reenvia os pixels do core para a textura existente (vídeo: um quadro por frame).
pub fn update(queue: &wgpu::Queue, img: *mut hg_image) {
    let p = unsafe { hg__image_gpu_data(img) } as *mut Texture;
    if p.is_null() { return }
    let (w, h) = image_size(img);
    let t = unsafe { &*p };
    // SAFETY: mesmo contrato de `create`: `width*height*4` bytes compactos.
    let pixels = unsafe { std::slice::from_raw_parts(hg__image_pixels(img), w as usize * h as usize * 4) };
    queue.write_texture(
        t.texture.as_image_copy(), pixels,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: None },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

/// Solta a textura de `img` (o wgpu adia a liberação real até a GPU terminar).
/// Devolve os bytes liberados.
pub fn destroy(img: *mut hg_image) -> u64 {
    let p = unsafe { hg__image_gpu_data(img) } as *mut Texture;
    if p.is_null() { return 0 }
    unsafe { hg__image_set_gpu_data(img, std::ptr::null_mut()) };
    unsafe { Box::from_raw(p) }.bytes
}

/// Instâncias e imagens do frame atual, em ordem de lote.
pub struct Images {
    pub instances: GrowBuffer,
    items: Vec<hg_image_draw>,
    images: Vec<*mut hg_image>,
    /// Soma dos bytes das texturas vivas neste device.
    pub texture_bytes: u64,
}

impl Images {
    pub fn new() -> Self {
        Images { instances: GrowBuffer::new("images"), items: Vec::new(), images: Vec::new(), texture_bytes: 0 }
    }

    pub fn gpu_bytes(&self) -> u64 {
        self.instances.capacity() + self.texture_bytes
    }

    /// Garante as texturas (via core → `texture_create`) e sobe as instâncias.
    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, dl: *const hg_draw_list) -> Res {
        let n = unsafe { hg__draw_list_image_count(dl) } as usize;
        self.items.clear();
        self.images.clear();
        if n == 0 { return Ok(()) }
        let r = unsafe { hg__draw_list_prepare_images(dl) };
        if r != HG_OK as hg_result { return Err(Fail(r, c"wgpu: textura de imagem falhou")) }
        self.items.reserve(n);
        self.images.reserve(n);
        // SAFETY: capacidade >= n e o C escreve exatamente n itens em cada vetor.
        unsafe {
            hg__draw_list_copy_images(dl, self.items.as_mut_ptr(), self.images.as_mut_ptr());
            self.items.set_len(n);
            self.images.set_len(n);
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.items.as_ptr().cast::<u8>(), n * size_of::<hg_image_draw>()) };
        self.instances.write(device, queue, bytes);
        Ok(())
    }

    /// Desenha `count` imagens a partir de `start`; vizinhas com a mesma imagem viram
    /// um único draw instanciado. Devolve o número de draws.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, pipes: &Pipelines, start: u32, count: u32) -> u32 {
        pass.set_pipeline(&pipes.image);
        pass.set_vertex_buffer(0, self.instances.get().unwrap().slice(..));
        let (mut calls, mut i) = (0, start);
        let end = start + count;
        while i < end {
            let img = self.images[i as usize];
            let mut j = i + 1;
            while j < end && self.images[j as usize] == img { j += 1 }
            // SAFETY: `prepare` garantiu a textura; ela só morre fora do submit.
            let tex = unsafe { &*(hg__image_gpu_data(img) as *const Texture) };
            pass.set_bind_group(1, &tex.group, &[]);
            pass.draw(0..4, i..j);
            calls += 1;
            i = j;
        }
        calls
    }
}
