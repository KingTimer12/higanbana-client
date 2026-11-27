//! Instance/adapter/device do WGPU: um `Gpu` por contexto, dono dos recursos
//! compartilhados entre janelas (pipelines, buffers de instância, atlas).

use super::buffers::GrowBuffer;
use super::glyphs::Glyphs;
use super::images::Images;
use super::pipeline::Pipelines;
use super::user_shader::UserShaders;
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
    pub images: Images,
    /// Shaders do usuário: layout comum + uniforms do quadro.
    pub user_shaders: UserShaders,
    pub readback: wgpu::Buffer,
}

/// Só um backend do wgpu: DX12, ou Vulkan se não houver runtime D3D12.
/// - Sem GL nem os outros: cada backend extra sobe um driver (dezenas de MB).
/// - DX12 antes de Vulkan: uma `VkInstance` do wgpu criada depois que o backend
///   Vulkan do core já criou/destruiu um device perde a configuração da superfície
///   (driver AMD compartilha estado entre instâncias) e o primeiro frame falha.
fn backends() -> wgpu::Backends {
    let d3d12: Vec<u16> = "d3d12.dll".encode_utf16().chain([0]).collect();
    if unsafe { hg__detect_dll(d3d12.as_ptr()) } != 0 { wgpu::Backends::DX12 } else { wgpu::Backends::VULKAN }
}

/// Instância única por processo, nunca destruída: o driver Vulkan da AMD
/// compartilha estado entre instâncias e destruir uma enquanto outra thread
/// apresenta derruba o processo (mesma correção de `vk_instance.c`).
fn instance() -> wgpu::Instance {
    static INSTANCE: std::sync::OnceLock<SharedInstance> = std::sync::OnceLock::new();
    INSTANCE
        .get_or_init(|| {
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = backends();
            SharedInstance(wgpu::Instance::new(desc))
        })
        .0
        .clone()
}

/// `wgpu::Instance` é Send + Sync em nativo (documentado), mas o rustc estoura o limite
/// de recursão provando o auto-trait no `Global` interno do wgpu
/// (`recursion_depth_exceeding_limit`). O newtype declara o Sync explicitamente.
struct SharedInstance(wgpu::Instance);
// SAFETY: wgpu nativo garante Instance: Send + Sync (feature `fragile-send-sync-non-atomic-wasm` não se aplica).
unsafe impl Send for SharedInstance {}
unsafe impl Sync for SharedInstance {}

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
            images: Images::new(),
            user_shaders: UserShaders::default(),
            readback,
        })
    }

    /// Memória de GPU estimada: buffers de instância + atlas + texturas de imagem
    /// (superfícies não contam).
    pub fn gpu_bytes(&self) -> u64 {
        self.quads.capacity() + self.glyphs.gpu_bytes() + self.images.gpu_bytes() + self.user_shaders.gpu_bytes()
    }
}
