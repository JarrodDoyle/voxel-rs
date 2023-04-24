use wgpu::util::DeviceExt;

use crate::render;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickmapUniform {
    pub bitmask: [u32; 16],
    pub shading_table_offset: u32,
    pub lod_color: u32,
}

impl BrickmapUniform {
    pub fn new() -> Self {
        Self {
            bitmask: [0; 16],
            shading_table_offset: 0,
            lod_color: 0,
        }
    }
}

#[derive(Debug)]
pub struct BrickmapManager {
    uniform: BrickmapUniform,
    buffer: wgpu::Buffer,
}

impl BrickmapManager {
    pub fn new(context: &render::Context) -> Self {
        let uniform = BrickmapUniform::new();
        let buffer = context
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

        Self { uniform, buffer }
    }

    pub fn set_data(&mut self, data: &[u32; 16]) {
        self.uniform.bitmask = *data;
    }

    pub fn update_buffer(&self, context: &render::Context) {
        context
            .queue
            .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }

    pub fn get_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}
