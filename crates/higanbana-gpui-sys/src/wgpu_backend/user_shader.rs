//! Shaders do usuário: o WGSL composto (prelúdio + usuário + `vs_main`/`ps_main`) vira
//! `create_shader_module` direto, sem tradução. Desenho = quad (strip de 4 vértices, sem
//! vertex buffer) com grupo 0 = uniform (binding 0, offset dinâmico), backdrop (1) e sampler (2).
//!
//! Os uniforms de todos os lotes de shader do quadro vão num único buffer (um slot alinhado
//! por desenho): `write_buffer` só roda antes do submit, então um slot compartilhado faria
//! todos os desenhos verem o último valor.

use super::device::{Gpu, SURFACE_FORMAT};
use super::surface::Window;
use super::Res;
use crate::*;
use std::ffi::CString;

const UNIFORM_BYTES: u64 = HG_SHADER_UNIFORM_BYTES as u64;
const UNIFORM_FLOATS: usize = HG_SHADER_UNIFORM_BYTES as usize / 4;

/// Objeto GPU de um `hg_shader` (atrás de `sh->gpu` como `Box` cru).
struct UserShader {
    pipeline: wgpu::RenderPipeline,
}

/// Um lote de shader do quadro atual, já resolvido em `upload`.
struct Draw {
    pipeline: wgpu::RenderPipeline,
    offset: u32,
    backdrop: bool,
    rect: hg_rect,
}

/// Estado comum, criado no 1º shader do contexto.
struct Common {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    /// Textura 1x1 transparente: ligada em binding 1 quando não há backdrop.
    empty: wgpu::TextureView,
    /// Uniforms do quadro (um slot de `stride` bytes por desenho); cresce sob demanda.
    uniforms: Option<wgpu::Buffer>,
    /// Grupo com `empty`; refeito quando `uniforms` é recriado.
    empty_group: Option<wgpu::BindGroup>,
    /// Muda a cada `uniforms` novo: os grupos de backdrop por janela comparam com ele.
    generation: u64,
}

#[derive(Default)]
pub struct UserShaders {
    common: Option<Common>,
    /// Lotes de shader do quadro, na ordem; `None` = lote pulado (falha ou rect vazio).
    draws: Vec<Option<Draw>>,
    scratch: Vec<u8>,
}

fn entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    let visibility = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

fn create_common(device: &wgpu::Device, queue: &wgpu::Queue) -> Common {
    let uniform = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: true,
        min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
    };
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    let sampler = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("user shader"),
        entries: &[entry(0, uniform), entry(1, texture), entry(2, sampler)],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0,
    });
    let size = wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("backdrop vazio"), size, mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2, format: SURFACE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
    });
    queue.write_texture(
        tex.as_image_copy(), &[0u8; 4],
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: None }, size,
    );
    Common {
        layout, pipeline_layout, empty: tex.create_view(&Default::default()),
        uniforms: None, empty_group: None, generation: 0,
    }
}

/// Grupo 0 com `view` em binding 1 (uniform com offset dinâmico, sampler linear/clamp).
pub(super) fn bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, uniforms: &wgpu::Buffer,
              view: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
    let ubo = wgpu::BufferBinding { buffer: uniforms, offset: 0, size: wgpu::BufferSize::new(UNIFORM_BYTES) };
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None, layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(ubo) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}

fn error_text(e: wgpu::Error) -> String {
    match e {
        wgpu::Error::Validation { description, .. } => description,
        other => other.to_string(),
    }
}

/// Erro do wgpu → `sh->error` (via core). Devolve o código que o core gravou.
fn set_error(sh: *mut hg_shader, msg: &str) -> hg_result {
    let msg = CString::new(msg.replace('\0', " ")).unwrap_or_default();
    unsafe { hg__shader_set_error(sh, msg.as_ptr()) }
}

/// `shader_prepare`: compila o WGSL composto e cria o pipeline. Erro de validação vira
/// `sh->error` (Ok com o código de erro); `Err` só para falhas internas.
pub fn prepare(gpu: &mut Gpu, sh: *mut hg_shader) -> Res<hg_result> {
    let src = unsafe { hg__shader_wgsl(sh) };
    if src.is_null() { return Ok(set_error(sh, "wgpu: shader sem fonte WGSL")) }
    let src = unsafe { std::ffi::CStr::from_ptr(src) }.to_string_lossy();
    let common = gpu.user_shaders.common.get_or_insert_with(|| create_common(&gpu.device, &gpu.queue));

    // Sem o escopo, o handler padrão do wgpu entra em pânico no erro de validação.
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("user shader"), source: wgpu::ShaderSource::Wgsl(src),
    });
    let target = [Some(wgpu::ColorTargetState {
        format: SURFACE_FORMAT,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("user shader"),
        layout: Some(&common.pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[],
        },
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module, entry_point: Some("ps_main"), compilation_options: Default::default(), targets: &target,
        }),
        multiview_mask: None,
        cache: None,
    });
    if let Some(e) = pollster::block_on(scope.pop()) {
        return Ok(set_error(sh, &format!("wgpu: {}", error_text(e))));
    }
    let gpu_obj = Box::new(UserShader { pipeline });
    unsafe { hg__shader_set_gpu_data(sh, Box::into_raw(gpu_obj).cast()) };
    Ok(HG_OK as hg_result)
}

/// `shader_release`: solta o pipeline (o wgpu adia a liberação até a GPU terminar).
pub fn release(sh: *mut hg_shader) {
    let p = unsafe { hg__shader_gpu_data(sh) } as *mut UserShader;
    unsafe { hg__shader_set_gpu_data(sh, std::ptr::null_mut()) };
    if !p.is_null() { drop(unsafe { Box::from_raw(p) }) }
}

impl UserShaders {
    pub fn gpu_bytes(&self) -> u64 {
        self.common.as_ref().and_then(|c| c.uniforms.as_ref()).map_or(0, |b| b.size())
    }

    /// Resolve os lotes de shader do quadro (compila no 1º uso) e sobe os uniforms de todos
    /// de uma vez. Sem lote de shader: só a iteração dos lotes, nada alocado.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, dl: *const hg_draw_list, w: u32, h: u32) {
        self.draws.clear();
        self.scratch.clear();
        let mut view: hg_batch_view = unsafe { std::mem::zeroed() };
        let mut cursor = std::ptr::null();
        let stride = UNIFORM_BYTES.next_multiple_of(device.limits().min_uniform_buffer_offset_alignment as u64) as usize;
        loop {
            cursor = unsafe { hg__draw_list_next_batch(dl, cursor, &mut view) };
            if cursor.is_null() { break }
            if view.shader.is_null() { continue }
            let draw = self.resolve(&view).map(|(pipeline, d)| {
                let offset = self.scratch.len();
                let mut u = [0f32; UNIFORM_FLOATS];
                unsafe { hg__shader_uniforms(d, w as f32, h as f32, 1.0, u.as_mut_ptr()) };
                self.scratch.extend(u.iter().flat_map(|f| f.to_ne_bytes()));
                self.scratch.resize(offset + stride, 0);
                Draw { pipeline, offset: offset as u32, backdrop: d.flags & HG_SHADER_BACKDROP != 0, rect: d.rect }
            });
            self.draws.push(draw);
        }
        if self.scratch.is_empty() { return }
        let Some(c) = self.common.as_mut() else { return };
        let need = self.scratch.len() as u64;
        if c.uniforms.as_ref().is_none_or(|b| b.size() < need) {
            c.uniforms = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("user shader uniforms"),
                size: need.next_power_of_two().max(stride as u64 * 4),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            c.empty_group = None;
            c.generation += 1;
        }
        queue.write_buffer(c.uniforms.as_ref().unwrap(), 0, &self.scratch);
    }

    /// Pipeline + parâmetros do lote, ou `None` (rect vazio ou shader que não compilou:
    /// o erro fica em `sh->error` e o quadro segue sem ele).
    fn resolve<'v>(&self, view: &'v hg_batch_view) -> Option<(wgpu::RenderPipeline, &'v hg_shader_draw)> {
        let d = unsafe { view.shader_draw.as_ref()? };
        let positive = |v: f32| v > 0.0; // NaN também fica de fora
        if !positive(d.rect.w) || !positive(d.rect.h) { return None }
        let sh = view.shader as *mut hg_shader;
        if unsafe { hg__shader_gpu(sh) } != HG_OK as hg_result { return None }
        let p = unsafe { hg__shader_gpu_data(sh) } as *const UserShader;
        // gpu NULL com HG_OK: o backend atual não preparou (não deve acontecer no WGPU).
        Some((unsafe { p.as_ref()? }.pipeline.clone(), d))
    }
}

/// Desenha o `index`-ésimo lote de shader do quadro. Com backdrop, encerra o pass, copia o
/// fundo e abre outro (Load). Devolve o pass em uso e se houve desenho; quem chama religa
/// o grupo 0 dos pipelines fixos.
pub fn draw(gpu: &mut Gpu, win: &mut Window, enc: &mut wgpu::CommandEncoder, mut pass: wgpu::RenderPass<'static>,
            index: usize, scissor: (u32, u32, u32, u32)) -> (wgpu::RenderPass<'static>, bool) {
    let shaders = &mut gpu.user_shaders;
    let (Some(Some(d)), Some(c)) = (shaders.draws.get(index), shaders.common.as_mut()) else { return (pass, false) };
    let Some(uniforms) = c.uniforms.as_ref() else { return (pass, false) };
    let mut captured = false;
    if d.backdrop {
        drop(pass);
        let size = win.size();
        let frame = win.frame.as_ref().map(|f| &f.texture);
        captured = frame.is_some_and(|f| win.backdrop.capture(&gpu.device, enc, f, size, &d.rect));
        pass = win.begin_pass(enc).forget_lifetime();
    }
    let group = if captured {
        win.backdrop.group(&gpu.device, &c.layout, uniforms, c.generation, &gpu.pipes.linear)
    } else {
        &*c.empty_group.get_or_insert_with(|| bind_group(&gpu.device, &c.layout, uniforms, &c.empty, &gpu.pipes.linear))
    };
    let (x, y, w, h) = scissor;
    pass.set_scissor_rect(x, y, w, h);
    pass.set_pipeline(&d.pipeline);
    pass.set_bind_group(0, group, &[d.offset]);
    pass.draw(0..4, 0..1);
    (pass, true)
}
