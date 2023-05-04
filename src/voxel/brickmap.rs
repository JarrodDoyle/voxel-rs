use wgpu::util::DeviceExt;

use crate::render;

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Brickmap {
    pub bitmask: [u32; 16],
    pub shading_table_offset: u32,
    pub lod_color: u32,
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
    brickgrid: Vec<u32>,
    brickgrid_buffer: wgpu::Buffer,
    brickmap_cache: Vec<Brickmap>,
    brickmap_cache_idx: usize,
    brickmap_buffer: wgpu::Buffer,
    shading_table: Vec<u32>,
    shading_table_buffer: wgpu::Buffer,
    feedback_buffer: wgpu::Buffer,
    feedback_result_buffer: wgpu::Buffer,
}

impl BrickmapManager {
    pub fn new(context: &render::Context) -> Self {
        let mut state_uniform = WorldState::default();
        state_uniform.brickmap_cache_dims = [32, 32, 32];

        let mut brickmap_cache = Vec::<Brickmap>::with_capacity(32768);
        brickmap_cache.resize(32768, Brickmap::default());

        let mut brickgrid = Vec::<u32>::with_capacity(32768);
        brickgrid.resize(brickgrid.capacity(), 1);

        let device = &context.device;
        let state_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[state_uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let brickgrid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&brickgrid),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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

        let mut arr = [0u32; 1028];
        arr[0] = 256;
        let feedback_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&arr),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let feedback_result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 1028 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            state_uniform,
            state_buffer,
            brickgrid,
            brickgrid_buffer,
            brickmap_cache,
            brickmap_cache_idx: 0,
            brickmap_buffer,
            shading_table,
            shading_table_buffer,
            feedback_buffer,
            feedback_result_buffer,
        }
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
        queue.write_buffer(
            &self.brickgrid_buffer,
            0,
            bytemuck::cast_slice(&self.brickgrid),
        )
    }

    pub fn get_brickgrid_buffer(&self) -> &wgpu::Buffer {
        &self.brickgrid_buffer
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

    pub fn get_feedback_buffer(&self) -> &wgpu::Buffer {
        &self.feedback_buffer
    }

    pub fn get_feedback_result_buffer(&self) -> &wgpu::Buffer {
        &self.feedback_result_buffer
    }

    // TODO: Implement an upload buffer that's unpacked on shader side
    pub fn process_feedback_buffer(&mut self, context: &render::Context) {
        // Get request count
        let mut slice = self.feedback_result_buffer.slice(0..16);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        context.device.poll(wgpu::Maintain::Wait);
        let mut data: Vec<u32> = bytemuck::cast_slice(slice.get_mapped_range().as_ref()).to_vec();
        self.feedback_result_buffer.unmap();

        let request_count = data[1] as usize;
        if request_count == 0 {
            return;
        }

        // Get the position data
        slice = self.feedback_result_buffer.slice(16..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        context.device.poll(wgpu::Maintain::Wait);
        data = bytemuck::cast_slice(slice.get_mapped_range().as_ref()).to_vec();
        self.feedback_result_buffer.unmap();

        // Generate a sphere of voxels
        let sphere_center = glam::vec3(3.5, 3.5, 3.5);
        let sphere_r2 = u32::pow(4, 2) as f32;
        for i in 0..request_count {
            let chunk_x = data[i * 4];
            let chunk_y = data[i * 4 + 1];
            let chunk_z = data[i * 4 + 2];

            let chunk_idx = (chunk_x + chunk_y * 32 + chunk_z * 1024) as usize;
            if chunk_idx % 3 == 0 || chunk_idx % 5 == 0 || chunk_idx % 7 == 0 {
                self.update_brickgrid_element(context, chunk_idx, 0)
            } else {
                // Build the voxel data
                let mut bitmask_data = [0xFFFFFFFF as u32; 16];
                let mut albedo_data = Vec::<u32>::new();
                for z in 0..8 {
                    let mut entry = 0u64;
                    for y in 0..8 {
                        for x in 0..8 {
                            let idx = x + y * 8;
                            let pos = glam::vec3(x as f32, y as f32, z as f32);
                            if (pos - sphere_center).length_squared() <= sphere_r2 {
                                entry += 1 << idx;
                                let mut albedo = 0u32;
                                albedo += ((x + 1) * 32 - 1) << 24;
                                albedo += ((y + 1) * 32 - 1) << 16;
                                albedo += ((z + 1) * 32 - 1) << 8;
                                albedo += 255;
                                albedo_data.push(albedo);
                            }
                        }
                    }
                    bitmask_data[2 * z as usize] = (entry & 0xFFFFFFFF).try_into().unwrap();
                    bitmask_data[2 * z as usize + 1] =
                        ((entry >> 32) & 0xFFFFFFFF).try_into().unwrap();
                }

                // Update the brickgrid index
                let brickgrid_element = ((self.brickmap_cache_idx as u32) << 8) + 4;
                self.update_brickgrid_element(context, chunk_idx, brickgrid_element);

                // Update the shading table
                let shading_idx = chunk_idx * 512;
                self.shading_table.splice(
                    shading_idx..(shading_idx + albedo_data.len()),
                    albedo_data.clone(),
                );
                context.queue.write_buffer(
                    &self.shading_table_buffer,
                    (shading_idx * 4) as u64,
                    bytemuck::cast_slice(&albedo_data),
                );

                // Update the brickmap
                self.brickmap_cache[self.brickmap_cache_idx].bitmask = bitmask_data;
                self.brickmap_cache[self.brickmap_cache_idx].shading_table_offset =
                    shading_idx as u32;
                context.queue.write_buffer(
                    &self.brickmap_buffer,
                    (72 * self.brickmap_cache_idx) as u64,
                    bytemuck::cast_slice(&[self.brickmap_cache[self.brickmap_cache_idx]]),
                );
                self.brickmap_cache_idx += 1;
            }
        }

        // Reset the request count on the gpu buffer
        let data = &[0, 0, 0, 0];
        context.queue.write_buffer(&self.feedback_buffer, 4, data);

        log::info!("Num loaded brickmaps: {}", self.brickmap_cache_idx);
    }

    fn update_brickgrid_element(&mut self, context: &render::Context, index: usize, data: u32) {
        self.brickgrid.splice(index..index + 1, [data]);
        context.queue.write_buffer(
            &self.brickgrid_buffer,
            (index * 4).try_into().unwrap(),
            bytemuck::cast_slice(&[self.brickgrid[index]]),
        );
    }
}
