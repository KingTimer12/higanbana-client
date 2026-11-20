//! Instance/adapter/device do WGPU: um `Gpu` por contexto, dono dos recursos
//! compartilhados entre janelas (pipelines, buffers de instância, atlas).

use super::buffers::GrowBuffer;
use super::glyphs::Glyphs;
use super::pipeline::Pipelines;
use super::{fail, Res};
use crate::*;

/// Formato do backbuffer: BGRA não-sRGB, igual à saída do D3D11.
pub const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
/// Buffer de leitura de 1 pixel (linha alinhada a 256 bytes).
pub const READBACK_BYTES: u64 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64;

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub pipes: Pipelines,
    pub quads: GrowBuffer,
    /// Rascunho reaproveitado para juntar os quads da draw list antes do upload.
    pub quad_scratch: Vec<hg_quad>,
    pub glyphs: Glyphs,
    pub readback: wgpu::Buffer,
}

/// Instância única por processo, nunca destruída: o driver Vulkan da AMD
/// compartilha estado entre instâncias e destruir uma enquanto outra thread
/// apresenta derruba o processo (mesma correção de `vk_instance.c`).
fn instance() -> wgpu::Instance {
    static INSTANCE: std::sync::OnceLock<wgpu::Instance> = std::sync::OnceLock::new();
    INSTANCE
        .get_or_init(|| wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle()))
        .clone()
}

fn request_adapter(instance: &wgpu::Instance) -> Option<wgpu::Adapter> {
    let opts = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    };
    pollster::block_on(instance.request_adapter(&opts)).ok()
}

/// Copia o nome do adaptador para `info.adapter` (UTF-8, truncado, terminado em 0).
fn write_adapter_name(adapter: &wgpu::Adapter, info: &mut hg_backend_info) {
    let name = adapter.get_info().name;
    let mut n = name.len().min(info.adapter.len() - 1);
    while !name.is_char_boundary(n) { n -= 1 }
    for (dst, src) in info.adapter.iter_mut().zip(name.as_bytes()[..n].iter()) {
        *dst = *src as _;
    }
    info.adapter[n] = 0;
}

impl Gpu {
    /// Há adaptador WGPU nesta máquina? Não deixa nada vivo.
    pub fn probe(info: Option<&mut hg_backend_info>) -> bool {
        let Some(adapter) = request_adapter(&instance()) else { return false };
        if let Some(info) = info { write_adapter_name(&adapter, info) }
        true
    }

    pub fn new(info: &mut hg_backend_info) -> Res<Self> {
        let instance = instance();
        let Some(adapter) = request_adapter(&instance) else {
            return fail(HG_ERR_UNAVAILABLE, c"wgpu: nenhum adaptador");
        };
        write_adapter_name(&adapter, info);
        let desc = wgpu::DeviceDescriptor {
            label: Some("higanbana"),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&desc)) else {
            return fail(HG_ERR_DEVICE, c"wgpu: request_device falhou");
        };
        let pipes = Pipelines::new(&device);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: READBACK_BYTES,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Gpu {
            instance, adapter, device, queue, pipes,
            quads: GrowBuffer::new("quads"),
            quad_scratch: Vec::new(),
            glyphs: Glyphs::new()?,
            readback,
        })
    }

    /// Memória de GPU estimada: buffers de instância + atlas (superfícies não contam).
    pub fn gpu_bytes(&self) -> u64 {
        self.quads.capacity() + self.glyphs.gpu_bytes()
    }
}
