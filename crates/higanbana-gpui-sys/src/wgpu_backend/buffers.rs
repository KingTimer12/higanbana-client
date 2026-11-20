//! Buffer de vértices por instância que só cresce (potência de 2) e é reaproveitado.

pub struct GrowBuffer {
    label: &'static str,
    buffer: Option<wgpu::Buffer>,
}

impl GrowBuffer {
    pub const fn new(label: &'static str) -> Self {
        GrowBuffer { label, buffer: None }
    }

    pub fn capacity(&self) -> u64 {
        self.buffer.as_ref().map_or(0, |b| b.size())
    }

    pub fn get(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Sobe `data` no início do buffer, recriando-o maior se não couber.
    pub fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) {
        let bytes = data.len() as u64;
        if bytes == 0 { return }
        if bytes > self.capacity() {
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: bytes.next_power_of_two().max(4096),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(self.buffer.as_ref().unwrap(), 0, data);
    }
}
