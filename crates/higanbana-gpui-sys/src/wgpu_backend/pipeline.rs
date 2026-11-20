//! Shaders WGSL, layouts e pipelines fixos (quads e glyphs) + uniform `Globals`.

use super::device::SURFACE_FORMAT;
use crate::hg_quad;

/// Tamanho de `hg_glyph_instance` (rect, uv, color).
pub const GLYPH_STRIDE: u64 = 48;
const QUAD_STRIDE: u64 = std::mem::size_of::<hg_quad>() as u64;
const _: () = assert!(QUAD_STRIDE == 88, "layout de hg_quad mudou: atualizar quad.wgsl");

const QUAD_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x2, 5 => Float32x4
];
const GLYPH_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4];

pub struct Pipelines {
    pub quad: wgpu::RenderPipeline,
    pub glyph: wgpu::RenderPipeline,
    pub atlas_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,
    /// Uniform: viewport (w, h) + atlas (w, h), reescrito a cada submit.
    pub globals: wgpu::Buffer,
    pub globals_group: wgpu::BindGroup,
}

fn layout_entry(binding: u32, ty: wgpu::BindingType, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: vis, ty, count: None }
}

fn pipeline(device: &wgpu::Device, src: &str, layouts: &[Option<&wgpu::BindGroupLayout>],
            stride: u64, attrs: &[wgpu::VertexAttribute]) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(src.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None, bind_group_layouts: layouts, immediate_size: 0,
    });
    let buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: stride, step_mode: wgpu::VertexStepMode::Instance, attributes: attrs,
    })];
    let target = [Some(wgpu::ColorTargetState {
        format: SURFACE_FORMAT,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &buffers,
        },
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module, entry_point: Some("fs_main"), compilation_options: Default::default(), targets: &target,
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Pipelines {
    pub fn new(device: &wgpu::Device) -> Self {
        let uniform = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None,
        };
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None, entries: &[layout_entry(0, uniform, wgpu::ShaderStages::VERTEX)],
        });
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2, multisampled: false,
        };
        let sampler_ty = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[layout_entry(0, texture, wgpu::ShaderStages::FRAGMENT),
                       layout_entry(1, sampler_ty, wgpu::ShaderStages::FRAGMENT)],
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"), size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
        });
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None, layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let quad = pipeline(device, include_str!("quad.wgsl"), &[Some(&globals_layout)], QUAD_STRIDE, &QUAD_ATTRS);
        let glyph = pipeline(device, include_str!("glyph.wgsl"), &[Some(&globals_layout), Some(&atlas_layout)],
                             GLYPH_STRIDE, &GLYPH_ATTRS);
        // Point sampling + clamp (os padrões do descriptor).
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
        Pipelines { quad, glyph, atlas_layout, sampler, globals, globals_group }
    }
}
