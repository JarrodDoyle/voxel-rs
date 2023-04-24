use std::mem::size_of;

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
    shading_table: Vec<u32>,
    shading_table_buffer: wgpu::Buffer,
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

        let shading_table_buffer =
            context
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: &[0; 25000000],
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });

        let shading_table = Vec::<u32>::new();

        Self {
            uniform,
            buffer,
            shading_table,
            shading_table_buffer,
        }
    }

    // TODO: Ideally this should take a generic voxel format and do the data mapping here
    pub fn set_data(&mut self, data: &[u32; 16], colours: &[u32]) {
        self.uniform.bitmask = *data;
        self.shading_table = colours.to_vec();
    }

    pub fn update_buffer(&self, context: &render::Context) {
        let queue = &context.queue;
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
        queue.write_buffer(
            &self.shading_table_buffer,
            0,
            bytemuck::cast_slice(&self.shading_table),
        );
    }

    pub fn get_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn get_shading_buffer(&self) -> &wgpu::Buffer {
        &self.shading_table_buffer
    }
}
