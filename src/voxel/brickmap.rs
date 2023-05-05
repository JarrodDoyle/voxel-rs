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
    shading_table_allocator: ShadingTableAllocator,
    feedback_buffer: wgpu::Buffer,
    feedback_result_buffer: wgpu::Buffer,
}

// TODO:
// - Proper shader table bucket management
// - GPU side unpack buffer rather than uploading each changed brickmap part
// - Cyclic brickmap cache with unloading
// - Brickworld system
// - Move terrain generation to it's own system
impl BrickmapManager {
    pub fn new(context: &render::Context) -> Self {
        let mut state_uniform = WorldState::default();
        state_uniform.brickmap_cache_dims = [32, 32, 32];

        let mut brickmap_cache = Vec::<Brickmap>::with_capacity(usize::pow(32, 3));
        brickmap_cache.resize(brickmap_cache.capacity(), Brickmap::default());

        let mut brickgrid = Vec::<u32>::with_capacity(usize::pow(32, 3));
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

        let shading_table_allocator = ShadingTableAllocator::new(4, u32::pow(2, 24));
        let shading_table_element_count = shading_table_allocator.total_elements as usize;
        let mut shading_table = Vec::<u32>::with_capacity(shading_table_element_count);
        shading_table.resize(shading_table.capacity(), 0);
        let shading_table_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&shading_table),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

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
            shading_table_allocator,
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
        let world_dims = self.state_uniform.brickmap_cache_dims;
        let sphere_center = glam::vec3(3.5, 3.5, 3.5);
        let sphere_r2 = u32::pow(4, 2) as f32;
        for i in 0..request_count {
            let chunk_x = data[i * 4];
            let chunk_y = data[i * 4 + 1];
            let chunk_z = data[i * 4 + 2];

            let chunk_idx = (chunk_x
                + chunk_y * world_dims[0]
                + chunk_z * world_dims[0] * world_dims[1]) as usize;
            if chunk_idx % 3 == 0 || chunk_idx % 5 == 0 || chunk_idx % 7 == 0 {
                self.update_brickgrid_element(context, chunk_idx, 0)
            } else {
                // Generate full data
                let mut chunk = [(false, 0u32); 512];
                for z in 0..8 {
                    for y in 0..8 {
                        for x in 0..8 {
                            let idx = (x + y * 8 + z * 8 * 8) as usize;

                            // Just checks if the point is in the sphere
                            let pos = glam::vec3(x as f32, y as f32, z as f32);
                            if (pos - sphere_center).length_squared() <= sphere_r2 {
                                // Pack the local position as a colour
                                let mut albedo = 0u32;
                                albedo += ((x + 1) * 32 - 1) << 24;
                                albedo += ((y + 1) * 32 - 1) << 16;
                                albedo += ((z + 1) * 32 - 1) << 8;
                                albedo += 255;
                                chunk[idx] = (true, albedo);
                            }
                        }
                    }
                }

                // Cull interior voxels
                let mut bitmask_data = [0xFFFFFFFF as u32; 16];
                let mut albedo_data = Vec::<u32>::new();
                for z in 0..8 {
                    // Each z level contains two bitmask segments of voxels
                    let mut entry = 0u64;
                    for y in 0..8 {
                        for x in 0..8 {
                            // Ignore non-solids
                            let idx = x + y * 8 + z * 8 * 8;
                            if !chunk[idx].0 {
                                continue;
                            }

                            // A voxel is on the surface if at least one of it's
                            // cardinal neighbours is non-solid. Also for simplicity if
                            // it's on the edge of the chunk
                            let surface_voxel: bool;
                            if x == 0 || x == 7 || y == 0 || y == 7 || z == 0 || z == 7 {
                                surface_voxel = true;
                            } else {
                                surface_voxel = !(chunk[idx + 1].0
                                    && chunk[idx - 1].0
                                    && chunk[idx + 8].0
                                    && chunk[idx - 8].0
                                    && chunk[idx + 64].0
                                    && chunk[idx - 64].0);
                            }

                            // Set the appropriate bit in the z entry and add the shading
                            // data
                            if surface_voxel {
                                entry += 1 << (x + y * 8);
                                albedo_data.push(chunk[idx].1);
                            }
                        }
                    }
                    let offset = 2 * z as usize;
                    bitmask_data[offset] = (entry & 0xFFFFFFFF).try_into().unwrap();
                    bitmask_data[offset + 1] = ((entry >> 32) & 0xFFFFFFFF).try_into().unwrap();
                }

                // Update the brickgrid index
                let brickgrid_element = ((self.brickmap_cache_idx as u32) << 8) + 4;
                self.update_brickgrid_element(context, chunk_idx, brickgrid_element);

                // Update the shading table
                let shading_idx = self
                    .shading_table_allocator
                    .try_alloc(albedo_data.len() as u32)
                    .unwrap() as usize;
                // let shading_idx = self.brickmap_cache_idx * 512;
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

#[derive(Debug)]
struct ShadingBucket {
    global_offset: u32,
    slot_count: u32,
    slot_size: u32,
    free: Vec<u32>,
    used: Vec<u32>,
}

impl ShadingBucket {
    pub fn new(global_offset: u32, slot_count: u32, slot_size: u32) -> Self {
        let mut free = Vec::with_capacity(slot_count as usize);
        for i in (0..slot_count).rev() {
            free.push(i);
        }

        let used = Vec::with_capacity(slot_count as usize);
        Self {
            global_offset,
            slot_count,
            slot_size,
            free,
            used,
        }
    }

    pub fn contains_address(&self, address: u32) -> bool {
        let min = self.global_offset;
        let max = min + self.slot_count * self.slot_size;
        return min <= address && address < max;
    }

    pub fn try_alloc(&mut self) -> Option<u32> {
        // Mark the first free index as used
        let bucket_index = self.free.pop()?;
        self.used.push(bucket_index);

        // Convert the bucket index into a global address
        let address = self.global_offset + bucket_index * self.slot_size;
        return Some(address);
    }

    pub fn try_dealloc(&mut self, address: u32) -> Result<(), &str> {
        if !self.contains_address(address) {
            return Err("Address is not within bucket range.");
        }

        let local_address = address - self.global_offset;
        if local_address % self.slot_size != 0 {
            return Err("Address is not aligned to bucket element size.");
        }

        let bucket_index = local_address / self.slot_size;
        if !self.used.contains(&bucket_index) {
            return Err("Address is not currently allocated.");
        }

        // All the potential errors are out of the way, time to actually deallocate
        let position = self.used.iter().position(|x| *x == bucket_index).unwrap();
        self.used.swap_remove(position);
        self.free.push(bucket_index);
        Ok(())
    }
}

#[derive(Debug)]
struct ShadingTableAllocator {
    buckets: Vec<ShadingBucket>,
    bucket_count: u32,
    elements_per_bucket: u32,
    total_elements: u32,
    used_elements: u32,
}

impl ShadingTableAllocator {
    pub fn new(bucket_count: u32, elements_per_bucket: u32) -> Self {
        let total_elements = bucket_count * elements_per_bucket;
        let used_elements = 0;

        // Build the buckets. Ordered in ascending size
        let mut buckets = Vec::with_capacity(bucket_count as usize);
        for i in (0..bucket_count).rev() {
            let global_offset = i * elements_per_bucket;
            let slot_size = u32::pow(2, 9 - i);
            let slot_count = elements_per_bucket / slot_size;
            log::info!(
                "Creating bucket: offset({}), slot_size({}), slot_count({})",
                global_offset,
                slot_size,
                slot_count
            );
            buckets.push(ShadingBucket::new(global_offset, slot_count, slot_size));
        }

        Self {
            buckets,
            bucket_count,
            elements_per_bucket,
            total_elements,
            used_elements,
        }
    }

    pub fn try_alloc(&mut self, size: u32) -> Option<u32> {
        for i in 0..self.bucket_count as usize {
            let bucket = &mut self.buckets[i];
            if bucket.slot_size < size {
                continue;
            }

            let idx = bucket.try_alloc();
            if idx.is_some() {
                self.used_elements += bucket.slot_size;
                log::info!(
                    "Allocated to shader table at {}. {}/{}",
                    idx.unwrap(),
                    self.used_elements,
                    self.total_elements
                );
                return idx;
            }
        }

        None
    }

    pub fn try_dealloc(&mut self, address: u32) -> Result<(), &str> {
        let bucket_idx = address / self.elements_per_bucket;
        let bucket = &mut self.buckets[bucket_idx as usize];
        self.used_elements -= bucket.slot_size;
        bucket.try_dealloc(address)
    }
}
