use std::collections::HashSet;

use wgpu::util::DeviceExt;

use crate::{math, render};

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
    brickgrid_dims: [u32; 3],
    _pad: u32,
}

#[derive(Debug, Default, Copy, Clone)]
struct BrickmapCacheEntry {
    grid_idx: usize,
    shading_table_offset: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BrickmapUnpackElement {
    cache_idx: u32,
    brickmap: Brickmap,
    shading_element_count: u32,
    shading_elements: [u32; 512],
}

enum BrickgridFlag {
    Empty = 0,
    Unloaded = 1,
    Loading = 2,
    Loaded = 4,
}

#[derive(Debug)]
pub struct BrickmapManager {
    state_uniform: WorldState,
    state_buffer: wgpu::Buffer,
    brickgrid: Vec<u32>,
    brickgrid_buffer: wgpu::Buffer,
    brickmap_cache_map: Vec<Option<BrickmapCacheEntry>>,
    brickmap_cache_idx: usize,
    brickmap_buffer: wgpu::Buffer,
    shading_table_buffer: wgpu::Buffer,
    shading_table_allocator: ShadingTableAllocator,
    feedback_buffer: wgpu::Buffer,
    feedback_result_buffer: wgpu::Buffer,
    brickgrid_staged: HashSet<usize>,
    brickgrid_unpack_buffer: wgpu::Buffer,
    brickmap_staged: Vec<BrickmapUnpackElement>,
    brickmap_unpack_buffer: wgpu::Buffer,
}

// TODO:
// - GPU side unpack buffer rather than uploading each changed brickmap part. HIGH PRIO!!
// - Brickworld system
impl BrickmapManager {
    pub fn new(context: &render::Context, brickgrid_dims: glam::UVec3) -> Self {
        let device = &context.device;

        let state_uniform = WorldState {
            brickgrid_dims: [brickgrid_dims.x, brickgrid_dims.y, brickgrid_dims.z],
            ..Default::default()
        };
        let state_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[state_uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let brickgrid =
            vec![1u32; (brickgrid_dims.x * brickgrid_dims.y * brickgrid_dims.z) as usize];
        let brickgrid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&brickgrid),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let brickmap_cache = vec![Brickmap::default(); usize::pow(32, 3)];
        let brickmap_cache_map = vec![None; brickmap_cache.capacity()];
        let brickmap_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&brickmap_cache),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let shading_table_allocator = ShadingTableAllocator::new(4, u32::pow(2, 24));
        let shading_table = vec![0u32; shading_table_allocator.total_elements as usize];
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

        let mut arr = vec![0u32; 516];
        arr[0] = 256;
        let brickgrid_staged = HashSet::new();
        let brickgrid_unpack_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&arr),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });

        let mut arr = vec![0u32; 136196];
        arr[0] = 256;
        let brickmap_staged = Vec::new();
        let brickmap_unpack_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&arr),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            state_uniform,
            state_buffer,
            brickgrid,
            brickgrid_buffer,
            brickmap_cache_map,
            brickmap_cache_idx: 0,
            brickmap_buffer,
            shading_table_buffer,
            shading_table_allocator,
            feedback_buffer,
            feedback_result_buffer,
            brickgrid_staged,
            brickgrid_unpack_buffer,
            brickmap_staged,
            brickmap_unpack_buffer,
        }
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

    pub fn get_brickmap_unpack_buffer(&self) -> &wgpu::Buffer {
        &self.brickmap_unpack_buffer
    }

    pub fn get_brickgrid_unpack_buffer(&self) -> &wgpu::Buffer {
        &self.brickgrid_unpack_buffer
    }

    pub fn process_feedback_buffer(
        &mut self,
        context: &render::Context,
        world: &mut super::world::WorldManager,
    ) {
        // Get request count
        let mut slice = self.feedback_result_buffer.slice(0..16);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        context.device.poll(wgpu::Maintain::Wait);
        let mut data: Vec<u32> = bytemuck::cast_slice(slice.get_mapped_range().as_ref()).to_vec();
        self.feedback_result_buffer.unmap();

        let request_count = data[1] as usize;
        if request_count == 0 {
            self.upload_unpack_buffers(context);
            return;
        }

        // Get the position data
        let range_end = 16 + 16 * request_count as u64;
        slice = self.feedback_result_buffer.slice(16..range_end);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        context.device.poll(wgpu::Maintain::Wait);
        data = bytemuck::cast_slice(slice.get_mapped_range().as_ref()).to_vec();
        self.feedback_result_buffer.unmap();

        // Generate a sphere of voxels
        let grid_dims = self.state_uniform.brickgrid_dims;
        for i in 0..request_count {
            // Extract brickgrid position of the requested brickmap
            let grid_pos = glam::uvec3(data[i * 4], data[i * 4 + 1], data[i * 4 + 2]);
            let grid_idx = math::to_1d_index(
                grid_pos,
                glam::uvec3(grid_dims[0], grid_dims[1], grid_dims[2]),
            );

            // The CPU side World uses different terminology and coordinate system
            // We need to convert between Brickmap and World pos and get the relevant
            // World voxels
            let block = Self::grid_pos_to_world_pos(world, grid_pos);

            // The World gives us the full voxel data for the requested block of voxels.
            // For Brickmap raytracing we only care about the visible surface voxels, so
            // we need to cull any interior voxels.
            let (bitmask_data, albedo_data) = Self::cull_interior_voxels(&block);

            // If there's no voxel colour data post-culling it means the brickmap is
            // empty. We don't need to upload it, just mark the relevant brickgrid entry.
            if albedo_data.is_empty() {
                self.update_brickgrid_element(grid_idx, 0);
                continue;
            }

            // Update the brickgrid index
            self.update_brickgrid_element(
                grid_idx,
                Self::to_brickgrid_element(self.brickmap_cache_idx as u32, BrickgridFlag::Loaded),
            );

            // If there's already something in the cache spot we want to write to, we
            // need to unload it.
            if self.brickmap_cache_map[self.brickmap_cache_idx].is_some() {
                let entry = self.brickmap_cache_map[self.brickmap_cache_idx].unwrap();
                self.update_brickgrid_element(entry.grid_idx, 1);
            }

            // TODO: Add to a brickmap unpack buffer
            // Update the shading table
            let shading_idx = self
                .shading_table_allocator
                .try_alloc(albedo_data.len() as u32)
                .unwrap() as usize;
            // context.queue.write_buffer(
            //     &self.shading_table_buffer,
            //     (shading_idx * 4) as u64,
            //     bytemuck::cast_slice(&albedo_data),
            // );

            // We're all good to overwrite the cache map entry now :)
            self.brickmap_cache_map[self.brickmap_cache_idx] = Some(BrickmapCacheEntry {
                grid_idx,
                shading_table_offset: shading_idx as u32,
            });

            // Update the brickmap
            let brickmap = Brickmap {
                bitmask: bitmask_data,
                shading_table_offset: shading_idx as u32,
                lod_color: 0,
            };
            // context.queue.write_buffer(
            //     &self.brickmap_buffer,
            //     (72 * self.brickmap_cache_idx) as u64,
            //     bytemuck::cast_slice(&[brickmap]),
            // );

            let shading_element_count = albedo_data.len();
            let mut shading_elements = [0u32; 512];
            shading_elements[..shading_element_count].copy_from_slice(&albedo_data);

            let staged_brickmap = BrickmapUnpackElement {
                cache_idx: self.brickmap_cache_idx as u32,
                brickmap,
                shading_element_count: shading_element_count as u32,
                shading_elements,
            };
            self.brickmap_staged.push(staged_brickmap);
            self.brickmap_cache_idx = (self.brickmap_cache_idx + 1) % self.brickmap_cache_map.len();
        }

        // Reset the request count on the gpu buffer
        let data = &[0, 0, 0, 0];
        context.queue.write_buffer(&self.feedback_buffer, 4, data);

        self.upload_unpack_buffers(context);

        // TODO: This is inaccurate if we've looped
        log::info!("Num loaded brickmaps: {}", self.brickmap_cache_idx);
    }

    fn update_brickgrid_element(&mut self, index: usize, data: u32) {
        // If we're updating a brickgrid element, we need to make sure to deallocate anything
        // that's already there. The shading table gets deallocated, and the brickmap cache entry
        // is marked as None.
        if (self.brickgrid[index] & 0xF) == 4 {
            let brickmap_idx = (self.brickgrid[index] >> 8) as usize;
            let cache_map_entry = self.brickmap_cache_map[brickmap_idx];
            match cache_map_entry {
                Some(entry) => {
                    match self
                        .shading_table_allocator
                        .try_dealloc(entry.shading_table_offset)
                    {
                        Ok(_) => (),
                        Err(e) => log::warn!("{}", e),
                    }
                    self.brickmap_cache_map[brickmap_idx] = None;
                }
                None => log::warn!("Expected brickmap cache entry, found None!"),
            }
        }

        // We're safe to overwrite the CPU brickgrid and mark for GPU upload now
        self.brickgrid[index] = data;
        self.brickgrid_staged.insert(index);
    }

    fn upload_unpack_buffers(&mut self, context: &render::Context) {
        // Brickgrid
        // TODO: Make this less shit??
        let mut data = Vec::new();
        let mut iter = self.brickgrid_staged.iter();
        let mut to_remove = Vec::new();
        for _ in 0..256 {
            let el = iter.next();
            if el.is_none() {
                break;
            }

            let val = el.unwrap();
            to_remove.push(*val as u32);
            data.push(*val as u32);
            data.push(self.brickgrid[*val]);
        }
        for val in &to_remove {
            self.brickgrid_staged.remove(&(*val as usize));
        }

        if !data.is_empty() {
            log::info!(
                "Uploading {} brickgrid entries. ({} remaining)",
                to_remove.len(),
                self.brickgrid_staged.len()
            );
        }

        context.queue.write_buffer(
            &self.brickgrid_unpack_buffer,
            4,
            bytemuck::cast_slice(&[data.len()]),
        );
        context.queue.write_buffer(
            &self.brickgrid_unpack_buffer,
            16,
            bytemuck::cast_slice(&data),
        );

        // Brickmap
        let end = 256.min(self.brickmap_staged.len());
        let iter = self.brickmap_staged.drain(0..end);
        let data = iter.as_slice();
        context.queue.write_buffer(
            &self.brickmap_unpack_buffer,
            4,
            bytemuck::cast_slice(&[end]),
        );
        context
            .queue
            .write_buffer(&self.brickmap_unpack_buffer, 16, bytemuck::cast_slice(data));
        drop(iter);

        if end > 0 {
            log::info!(
                "Uploading {} brickmap entries. ({} remaining)",
                end,
                self.brickmap_staged.len()
            );
        }
    }

    fn cull_interior_voxels(block: &[super::world::Voxel]) -> ([u32; 16], Vec<u32>) {
        let mut bitmask_data = [0xFFFFFFFF_u32; 16];
        let mut albedo_data = Vec::<u32>::new();
        for z in 0..8 {
            // Each z level contains two bitmask segments of voxels
            let mut entry = 0u64;
            for y in 0..8 {
                for x in 0..8 {
                    // Ignore non-solids
                    let idx = x + y * 8 + z * 8 * 8;
                    let empty_voxel = super::world::Voxel::Empty;

                    match block[idx] {
                        super::world::Voxel::Empty => continue,
                        super::world::Voxel::Color(r, g, b) => {
                            // A voxel is on the surface if at least one of it's
                            // cardinal neighbours is non-solid. Also for simplicity
                            // if it's on the edge of the chunk
                            // TODO: Account for neighbours in other blocks
                            let surface_voxel =
                                if x == 0 || x == 7 || y == 0 || y == 7 || z == 0 || z == 7 {
                                    true
                                } else {
                                    !(block[idx + 1] == empty_voxel
                                        && block[idx - 1] == empty_voxel
                                        && block[idx + 8] == empty_voxel
                                        && block[idx - 8] == empty_voxel
                                        && block[idx + 64] == empty_voxel
                                        && block[idx - 64] == empty_voxel)
                                };

                            // Set the appropriate bit in the z entry and add the
                            // shading data
                            if surface_voxel {
                                entry += 1 << (x + y * 8);
                                let albedo = ((r as u32) << 24)
                                    + ((g as u32) << 16)
                                    + ((b as u32) << 8)
                                    + 255u32;
                                albedo_data.push(albedo);
                            }
                        }
                    }
                }
            }
            let offset = 2 * z;
            bitmask_data[offset] = (entry & 0xFFFFFFFF).try_into().unwrap();
            bitmask_data[offset + 1] = ((entry >> 32) & 0xFFFFFFFF).try_into().unwrap();
        }
        (bitmask_data, albedo_data)
    }

    fn to_brickgrid_element(brickmap_cache_idx: u32, flags: BrickgridFlag) -> u32 {
        (brickmap_cache_idx << 8) + flags as u32
    }

    fn grid_pos_to_world_pos(
        world: &mut super::world::WorldManager,
        grid_pos: glam::UVec3,
    ) -> Vec<super::world::Voxel> {
        let chunk_dims = world.get_chunk_dims();
        let chunk_pos = glam::ivec3(
            (grid_pos.x / chunk_dims.x) as i32,
            (grid_pos.y / chunk_dims.y) as i32,
            (grid_pos.z / chunk_dims.z) as i32,
        );
        let block_pos = grid_pos % chunk_dims;
        let block = world.get_block(chunk_pos, block_pos);
        assert_eq!(block.len(), 512);
        block
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
    fn new(global_offset: u32, slot_count: u32, slot_size: u32) -> Self {
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

    fn contains_address(&self, address: u32) -> bool {
        let min = self.global_offset;
        let max = min + self.slot_count * self.slot_size;
        min <= address && address < max
    }

    fn try_alloc(&mut self) -> Option<u32> {
        // Mark the first free index as used
        let bucket_index = self.free.pop()?;
        self.used.push(bucket_index);

        // Convert the bucket index into a global address
        Some(self.global_offset + bucket_index * self.slot_size)
    }

    fn try_dealloc(&mut self, address: u32) -> Result<(), String> {
        log::trace!("Dealloc address: {}", address);
        if !self.contains_address(address) {
            let msg = format!("Address ({}) is not within bucket range.", address);
            return Err(msg);
        }

        let local_address = address - self.global_offset;
        if local_address % self.slot_size != 0 {
            return Err("Address is not aligned to bucket element size.".to_string());
        }

        let bucket_index = local_address / self.slot_size;
        if !self.used.contains(&bucket_index) {
            return Err("Address is not currently allocated.".to_string());
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
    fn new(bucket_count: u32, elements_per_bucket: u32) -> Self {
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

    fn try_alloc(&mut self, size: u32) -> Option<u32> {
        for i in 0..self.bucket_count as usize {
            let bucket = &mut self.buckets[i];
            if bucket.slot_size < size {
                continue;
            }

            let idx = bucket.try_alloc();
            if idx.is_some() {
                self.used_elements += bucket.slot_size;
                log::trace!(
                    "Allocated to shader table at {}. {}/{} ({}%)",
                    idx.unwrap(),
                    self.used_elements,
                    self.total_elements,
                    ((self.used_elements as f32 / self.total_elements as f32) * 100.0).floor()
                );
                return idx;
            }
        }

        None
    }

    fn try_dealloc(&mut self, address: u32) -> Result<(), String> {
        // Buckets are reverse order of their global offset so we need to reverse our idx
        let mut bucket_idx = address / self.elements_per_bucket;
        bucket_idx = self.bucket_count - bucket_idx - 1;
        let bucket = &mut self.buckets[bucket_idx as usize];
        self.used_elements -= bucket.slot_size;
        bucket.try_dealloc(address)
    }
}
