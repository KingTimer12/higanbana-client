//! Textura por superfície com o que já foi desenhado atrás de um shader. Só existe depois do
//! 1º shader com `HG_SHADER_BACKDROP`; cada desenho copia apenas o retângulo dele + margem
//! (entre dois render passes), então dois vidros sobrepostos veem um ao outro.

use super::device::SURFACE_FORMAT;
use super::user_shader::bind_group;
use crate::hg_rect;

/// Margem copiada em volta do retângulo do shader (pixels), para blur/refração.
const MARGIN: f32 = 64.0;

#[derive(Default)]
pub struct Backdrop {
    texture: Option<(wgpu::Texture, wgpu::TextureView)>,
    /// Grupo 0 dos shaders com esta textura + geração do buffer de uniforms usado.
    group: Option<(wgpu::BindGroup, u64)>,
}

fn clamp_px(v: f32, max: u32) -> u32 {
    // `as u32` satura: negativo e NaN viram 0.
    (v as u32).min(max)
}

fn at(texture: &wgpu::Texture, origin: wgpu::Origin3d) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin, aspect: wgpu::TextureAspect::All }
}

impl Backdrop {
    /// Solta a textura (resize/detach); a próxima captura recria no tamanho novo.
    pub fn release(&mut self) {
        self.group = None;
        self.texture = None;
    }

    /// Mesmo formato da superfície (`Bgra8Unorm`, não sRGB): a cópia exige formatos iguais.
    fn ensure(&mut self, device: &wgpu::Device, (w, h): (u32, u32)) {
        if self.texture.as_ref().is_some_and(|(t, _)| t.width() == w && t.height() == h) { return }
        self.release();
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("backdrop"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: SURFACE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.texture = Some((tex, view));
    }

    /// Copia `r` + margem (recortado à superfície) do frame para o backdrop. Fora do render
    /// pass. `false` se não há nada a copiar ou a superfície não aceita `COPY_SRC`.
    pub fn capture(&mut self, device: &wgpu::Device, enc: &mut wgpu::CommandEncoder, frame: &wgpu::Texture,
                   (w, h): (u32, u32), r: &hg_rect) -> bool {
        let x0 = clamp_px((r.x - MARGIN).floor(), w);
        let y0 = clamp_px((r.y - MARGIN).floor(), h);
        let x1 = clamp_px((r.x + r.w + MARGIN).ceil(), w);
        let y1 = clamp_px((r.y + r.h + MARGIN).ceil(), h);
        if x0 >= x1 || y0 >= y1 || !frame.usage().contains(wgpu::TextureUsages::COPY_SRC) { return false }
        // O frame pode diferir de `w`x`h` por um instante (resize pendente): recorta nele também.
        let (x1, y1) = (x1.min(frame.width()), y1.min(frame.height()));
        if x0 >= x1 || y0 >= y1 { return false }
        self.ensure(device, (w, h));
        let (tex, _) = self.texture.as_ref().unwrap();
        let origin = wgpu::Origin3d { x: x0, y: y0, z: 0 };
        enc.copy_texture_to_texture(at(frame, origin), at(tex, origin), wgpu::Extent3d {
            width: x1 - x0, height: y1 - y0, depth_or_array_layers: 1,
        });
        true
    }

    /// Grupo 0 com a textura do backdrop; refeito se a textura ou o buffer de uniforms mudou.
    /// Só depois de um `capture` que devolveu `true`.
    pub fn group(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout, uniforms: &wgpu::Buffer,
                 generation: u64, sampler: &wgpu::Sampler) -> &wgpu::BindGroup {
        let (_, view) = self.texture.as_ref().expect("backdrop sem textura");
        if self.group.as_ref().is_none_or(|(_, g)| *g != generation) {
            self.group = Some((bind_group(device, layout, uniforms, view, sampler), generation));
        }
        &self.group.as_ref().unwrap().0
    }
}
