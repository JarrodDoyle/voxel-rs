use wgpu::util::DeviceExt;

use crate::render;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Brickmap {
    pub bitmask: [u32; 16],
    pub shading_table_offset: u32,
    pub lod_color: u32,
}

impl Brickmap {
    pub fn new() -> Self {
        Self {
            bitmask: [0; 16],
            shading_table_offset: 0,
            lod_color: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct WorldState {
    brickmap_cache_dims: [u32; 3],
    _pad: u32,
}

#[derive(Debug)]
pub struct BrickmapManager {
    state_uniform: WorldState,
    state_buffer: wgpu::Buffer,
    brickmap_cache: Vec<Brickmap>,
    brickmap_buffer: wgpu::Buffer,
    shading_table: Vec<u32>,
    shading_table_buffer: wgpu::Buffer,
}

impl BrickmapManager {
    pub fn new(context: &render::Context) -> Self {
        let mut state_uniform = WorldState::default();
        state_uniform.brickmap_cache_dims = [32, 32, 32];

        let mut brickmap_cache = Vec::<Brickmap>::with_capacity(32768);
        brickmap_cache.resize(32768, Brickmap::default());

        let device = &context.device;
        let state_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[state_uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let brickmap_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&brickmap_cache),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let shading_table_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0u32; 25000000]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let mut shading_table = Vec::<u32>::with_capacity(25000000);
        shading_table.resize(shading_table.capacity(), 0);

        Self {
            state_uniform,
            state_buffer,
            brickmap_cache,
            brickmap_buffer,
            shading_table,
            shading_table_buffer,
        }
    }

    // TODO: Ideally this should take a generic voxel format and do the data mapping here
    pub fn set_data(&mut self, chunk_pos: glam::UVec3, data: &[u32; 16], colours: &[u32]) {
        let idx: usize = (chunk_pos.x + chunk_pos.y * 32 + chunk_pos.z * 1024)
            .try_into()
            .unwrap();
        let shading_idx = idx * 512;
        self.brickmap_cache[idx].bitmask = *data;
        self.brickmap_cache[idx].shading_table_offset = shading_idx as u32;
        self.shading_table.splice(
            shading_idx..(shading_idx + colours.len()),
            colours.to_owned(),
        );
    }

    pub fn update_buffer(&self, context: &render::Context) {
        let queue = &context.queue;
        queue.write_buffer(
            &self.brickmap_buffer,
            0,
            bytemuck::cast_slice(&self.brickmap_cache),
        );
        queue.write_buffer(
            &self.shading_table_buffer,
            0,
            bytemuck::cast_slice(&self.shading_table),
        );
    }

    pub fn get_worldstate_buffer(&self) -> &wgpu::Buffer {
        &self.state_buffer
    }

    pub fn get_brickmap_buffer(&self) -> &wgpu::Buffer {
        &self.brickmap_buffer
    }

    pub fn get_shading_buffer(&self) -> &wgpu::Buffer {
        &self.shading_table_buffer
    }
}
