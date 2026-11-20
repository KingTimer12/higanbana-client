//! Envio da draw list: sobe quads/glyphs/uniform e desenha um lote por clip
//! (quads, imagens e texto por cima), num único render pass.

use super::device::Gpu;
use super::surface::Window;
use super::Res;
use crate::*;

/// Junta os quads de todos os lotes no rascunho e sobe para o instance buffer.
fn upload_quads(gpu: &mut Gpu, dl: *const hg_draw_list) {
    let n = unsafe { hg_draw_list_quad_count(dl) } as usize;
    if n == 0 { return }
    let scratch = &mut gpu.quad_scratch;
    scratch.clear();
    scratch.reserve(n);
    // SAFETY: capacidade >= n e o C escreve exatamente n quads.
    unsafe {
        hg__draw_list_copy_quads(dl, scratch.as_mut_ptr());
        scratch.set_len(n);
    }
    let bytes = unsafe { std::slice::from_raw_parts(scratch.as_ptr().cast::<u8>(), n * size_of::<hg_quad>()) };
    gpu.quads.write(&gpu.device, &gpu.queue, bytes);
}

fn upload_globals(gpu: &Gpu, w: u32, h: u32) {
    let atlas = unsafe { hg__glyph_atlas_size() } as f32;
    let g = [w as f32, h as f32, atlas, atlas];
    let mut raw = [0u8; 16];
    for (i, v) in g.iter().enumerate() { raw[i * 4..i * 4 + 4].copy_from_slice(&v.to_ne_bytes()) }
    gpu.queue.write_buffer(&gpu.pipes.globals, 0, &raw);
}

/// Scissor do lote recortado à superfície; `None` se vazio.
fn scissor(b: &hg_batch_view, w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    if b.has_clip == 0 { return Some((0, 0, w, h)) }
    let clamp = |v: f32, max: u32| (v.max(0.0) as u32).min(max);
    let (x0, y0) = (clamp(b.clip.x, w), clamp(b.clip.y, h));
    let (x1, y1) = (clamp(b.clip.x + b.clip.w, w), clamp(b.clip.y + b.clip.h, h));
    (x1 > x0 && y1 > y0).then(|| (x0, y0, x1 - x0, y1 - y0))
}

pub fn draw_list(gpu: &mut Gpu, ctx: *mut hg_context, win: &mut Window, dl: *const hg_draw_list) -> Res {
    if win.view.is_none() { return Ok(()) } // frame pulado (oclusão/timeout)
    let (w, h) = win.size();
    gpu.glyphs.prepare(&gpu.device, &gpu.queue, &gpu.pipes, ctx, dl)?;
    gpu.images.prepare(&gpu.device, &gpu.queue, dl)?;
    upload_quads(gpu, dl);
    upload_globals(gpu, w, h);

    let stats = unsafe { &mut *hg__ctx_stats(ctx) };
    stats.draw_calls = 0;
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = win.begin_pass(&mut enc);
        pass.set_bind_group(0, &gpu.pipes.globals_group, &[]);
        let (mut quad_start, mut image_start, mut glyph_start, mut i) = (0u32, 0u32, 0u32, 0u32);
        let mut view = hg_batch_view { clip: hg_rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }, has_clip: 0, quad_count: 0, image_count: 0 };
        let mut cursor = std::ptr::null();
        loop {
            cursor = unsafe { hg__draw_list_next_batch(dl, cursor, &mut view) };
            if cursor.is_null() { break }
            let (quads, images, glyphs) = (view.quad_count, view.image_count, gpu.glyphs.batch_count(i));
            i += 1;
            if let Some((x, y, sw, sh)) = scissor(&view, w, h).filter(|_| quads + images + glyphs > 0) {
                pass.set_scissor_rect(x, y, sw, sh);
                if quads > 0 {
                    pass.set_pipeline(&gpu.pipes.quad);
                    pass.set_vertex_buffer(0, gpu.quads.get().unwrap().slice(..));
                    pass.draw(0..4, quad_start..quad_start + quads);
                    stats.draw_calls += 1;
                }
                if images > 0 {
                    stats.draw_calls += gpu.images.draw(&mut pass, &gpu.pipes, image_start, images);
                }
                if glyphs > 0 {
                    pass.set_pipeline(&gpu.pipes.glyph);
                    pass.set_bind_group(1, &gpu.glyphs.atlas.as_ref().unwrap().group, &[]);
                    pass.set_vertex_buffer(0, gpu.glyphs.instances.get().unwrap().slice(..));
                    pass.draw(0..4, glyph_start..glyph_start + glyphs);
                    stats.draw_calls += 1;
                }
            }
            quad_start += quads;
            image_start += images;
            glyph_start += glyphs;
        }
    }
    gpu.queue.submit([enc.finish()]);
    stats.gpu_bytes = gpu.gpu_bytes();
    Ok(())
}
