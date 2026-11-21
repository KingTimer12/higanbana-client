//! Leitura de um pixel do backbuffer (só testes): copia para um buffer MAP_READ e
//! bloqueia até o mapeamento. Chamado entre submit e end_frame.

use super::device::Gpu;
use super::surface::Window;
use super::{fail, Res};
use crate::*;

/// Pixel BGRA8 com B no byte baixo (igual ao D3D11).
pub fn pixel(gpu: &Gpu, win: &Window, x: u32, y: u32) -> Res<u32> {
    let Some(frame) = win.frame.as_ref() else { return fail(HG_ERR_INVALID, c"wgpu: sem frame ativo") };
    let (w, h) = win.size();
    if x >= w || y >= h { return fail(HG_ERR_INVALID, c"wgpu: pixel fora da superfície") }
    if !frame.texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
        return fail(HG_ERR_UNAVAILABLE, c"wgpu: superfície sem COPY_SRC");
    }
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &frame.texture, mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 }, aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &gpu.readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0, bytes_per_row: Some(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT), rows_per_image: None,
            },
        },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );
    gpu.queue.submit([enc.finish()]);
    let slice = gpu.readback.slice(..4);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let wait = wgpu::PollType::Wait { submission_index: None, timeout: None };
    if gpu.device.poll(wait).is_err() { return fail(HG_ERR_DEVICE, c"wgpu: poll falhou") }
    let px = match slice.get_mapped_range() {
        Ok(view) => u32::from_le_bytes([view[0], view[1], view[2], view[3]]),
        Err(_) => return fail(HG_ERR_DEVICE, c"wgpu: map do readback falhou"),
    };
    gpu.readback.unmap();
    Ok(px)
}
