use std::collections::HashSet;

use crate::{
    gfx::{self, BufferExt},
    math,
    voxel::world::{Voxel, WorldManager},
};

use super::shading_table::ShadingTableAllocator;

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
    _Empty = 0,
    _Unloaded = 1,
    _Loading = 2,
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
    unpack_max_count: usize,
    brickgrid_staged: HashSet<usize>,
    brickgrid_unpack_buffer: wgpu::Buffer,
    brickmap_staged: Vec<BrickmapUnpackElement>,
    brickmap_unpack_buffer: wgpu::Buffer,
}

// TODO:
// - Brickworld system
impl BrickmapManager {
    pub fn new(
        context: &gfx::Context,
        brickgrid_dims: glam::UVec3,
        brickmap_cache_size: usize,
        shading_table_bucket_size: u32,
        max_requested_brickmaps: u32,
        max_uploaded_brickmaps: u32,
    ) -> Self {
        let state_uniform = WorldState {
            brickgrid_dims: [brickgrid_dims.x, brickgrid_dims.y, brickgrid_dims.z],
            ..Default::default()
        };

        let brickgrid =
            vec![1u32; (brickgrid_dims.x * brickgrid_dims.y * brickgrid_dims.z) as usize];

        let brickmap_cache = vec![Brickmap::default(); brickmap_cache_size];
        let brickmap_cache_map = vec![None; brickmap_cache.capacity()];

        let shading_table_allocator = ShadingTableAllocator::new(4, shading_table_bucket_size);
        let shading_table = vec![0u32; shading_table_allocator.total_elements as usize];

        let mut feedback_data = vec![0u32; 4 + 4 * max_requested_brickmaps as usize];
        feedback_data[0] = max_requested_brickmaps;
        let feedback_data_u8 = bytemuck::cast_slice(&feedback_data);

        let mut brickgrid_upload_data = vec![0u32; 4 + 4 * max_uploaded_brickmaps as usize];
        brickgrid_upload_data[0] = max_uploaded_brickmaps;
        let brickgrid_staged = HashSet::new();

        let mut brickmap_upload_data = vec![0u32; 4 + 532 * max_uploaded_brickmaps as usize];
        brickmap_upload_data[0] = max_uploaded_brickmaps;
        let brickmap_staged = Vec::new();

        let mut buffers = gfx::BulkBufferBuilder::new()
            .with_init_buffer_bm("Brick World State", &[state_uniform])
            .set_usage(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
            .with_init_buffer_bm("Brickgrid", &brickgrid)
            .with_init_buffer_bm("Brickmap Cache", &brickmap_cache)
            .with_init_buffer_bm("Shading Table", &shading_table)
            .with_init_buffer_bm("Brickgrid Unpack", &brickgrid_upload_data)
            .with_init_buffer_bm("Brickmap Unpack", &brickmap_upload_data)
            .set_usage(
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            )
            .with_init_buffer("Feedback", feedback_data_u8)
            .set_usage(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ)
            .with_buffer("Feedback Read", feedback_data_u8.len() as u64, false)
            .build(context);

        Self {
            state_uniform,
            brickgrid,
            brickmap_cache_map,
            brickmap_cache_idx: 0,
            shading_table_allocator,
            unpack_max_count: max_uploaded_brickmaps as usize,
            brickgrid_staged,
            brickmap_staged,

            state_buffer: buffers.remove(0),
            brickgrid_buffer: buffers.remove(0),
            brickmap_buffer: buffers.remove(0),
            shading_table_buffer: buffers.remove(0),
            brickgrid_unpack_buffer: buffers.remove(0),
            brickmap_unpack_buffer: buffers.remove(0),
            feedback_buffer: buffers.remove(0),
            feedback_result_buffer: buffers.remove(0),
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

    pub fn get_unpack_max_count(&self) -> usize {
        self.unpack_max_count
    }

    pub fn process_feedback_buffer(&mut self, context: &gfx::Context, world: &mut WorldManager) {
        // Get request count
        let data: Vec<u32> = self.feedback_result_buffer.get_mapped_range(context, 0..16);
        let request_count = data[1] as usize;
        if request_count == 0 {
            self.upload_unpack_buffers(context);
            return;
        }

        // Get the position data
        let range = 16..(16 + 16 * request_count as u64);
        let data: Vec<u32> = self.feedback_result_buffer.get_mapped_range(context, range);

        // Generate a sphere of voxels
        let grid_dims = self.state_uniform.brickgrid_dims;
        for i in 0..request_count {
            // Extract brickgrid position of the requested brickmap
            let grid_pos = glam::uvec3(data[i * 4], data[i * 4 + 1], data[i * 4 + 2]);
            let grid_idx = math::to_1d_index(
                grid_pos,
                glam::uvec3(grid_dims[0], grid_dims[1], grid_dims[2]),
            );

            // We only want to upload voxels that are on the surface, so we cull anything
            // that is surrounded by solid voxels
            let grid_pos = grid_pos.as_ivec3();
            let (bitmask_data, albedo_data) = Self::cull_interior_voxels(world, grid_pos);

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

            // Update the shading table
            let shading_idx = self
                .shading_table_allocator
                .try_alloc(albedo_data.len() as u32)
                .unwrap() as usize;

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

    fn upload_unpack_buffers(&mut self, context: &gfx::Context) {
        // Brickgrid
        let mut data = Vec::new();
        let mut iter = self.brickgrid_staged.iter();
        let mut to_remove = Vec::new();
        for _ in 0..self.unpack_max_count {
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
        let end = self.unpack_max_count.min(self.brickmap_staged.len());
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

    fn cull_interior_voxels(
        world: &mut WorldManager,
        grid_pos: glam::IVec3,
    ) -> ([u32; 16], Vec<u32>) {
        // This is the data we want to return
        let mut bitmask_data = [0xFFFFFFFF_u32; 16];
        let mut albedo_data = Vec::<u32>::new();

        // Calculate world chunk and block positions for each that may be accessed
        let center_pos = Self::grid_pos_to_world_pos(world, grid_pos);
        let forward_pos = Self::grid_pos_to_world_pos(world, grid_pos + glam::ivec3(1, 0, 0));
        let backward_pos = Self::grid_pos_to_world_pos(world, grid_pos + glam::ivec3(-1, 0, 0));
        let left_pos = Self::grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, 0, -1));
        let right_pos = Self::grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, 0, 1));
        let up_pos = Self::grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, 1, 0));
        let down_pos = Self::grid_pos_to_world_pos(world, grid_pos + glam::ivec3(0, -1, 0));

        // Fetch those blocks
        let center_block = world.get_block(center_pos.0, center_pos.1);
        let forward_block = world.get_block(forward_pos.0, forward_pos.1);
        let backward_block = world.get_block(backward_pos.0, backward_pos.1);
        let left_block = world.get_block(left_pos.0, left_pos.1);
        let right_block = world.get_block(right_pos.0, right_pos.1);
        let up_block = world.get_block(up_pos.0, up_pos.1);
        let down_block = world.get_block(down_pos.0, down_pos.1);

        //  Reusable array of whether cardinal neighbours are empty
        let mut neighbours = [false; 6];
        for z in 0..8 {
            // Each z level contains two bitmask segments of voxels
            let mut entry = 0u64;
            for y in 0..8 {
                for x in 0..8 {
                    // Ignore non-solids
                    let idx = x + y * 8 + z * 8 * 8;
                    let empty_voxel = Voxel::Empty;

                    match center_block[idx] {
                        Voxel::Empty => continue,
                        Voxel::Color(r, g, b) => {
                            // A voxel is on the surface if at least one of it's
                            // cardinal neighbours is non-solid.
                            neighbours[0] = if x == 7 {
                                forward_block[idx - 7] == empty_voxel
                            } else {
                                center_block[idx + 1] == empty_voxel
                            };

                            neighbours[1] = if x == 0 {
                                backward_block[idx + 7] == empty_voxel
                            } else {
                                center_block[idx - 1] == empty_voxel
                            };

                            neighbours[2] = if z == 7 {
                                right_block[idx - 448] == empty_voxel
                            } else {
                                center_block[idx + 64] == empty_voxel
                            };

                            neighbours[3] = if z == 0 {
                                left_block[idx + 448] == empty_voxel
                            } else {
                                center_block[idx - 64] == empty_voxel
                            };

                            neighbours[4] = if y == 7 {
                                up_block[idx - 56] == empty_voxel
                            } else {
                                center_block[idx + 8] == empty_voxel
                            };

                            neighbours[5] = if y == 0 {
                                down_block[idx + 56] == empty_voxel
                            } else {
                                center_block[idx - 8] == empty_voxel
                            };

                            // Set the appropriate bit in the z entry and add the
                            // shading data
                            let surface_voxel = neighbours.iter().any(|v| *v);
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
        world: &mut WorldManager,
        grid_pos: glam::IVec3,
    ) -> (glam::IVec3, glam::UVec3) {
        // We deal with dvecs here because we want a negative grid_pos to have floored
        // chunk_pos
        let chunk_dims = world.get_chunk_dims().as_dvec3();
        let chunk_pos = (grid_pos.as_dvec3() / chunk_dims).floor();
        let block_pos = grid_pos - (chunk_pos * chunk_dims).as_ivec3();
        (chunk_pos.as_ivec3(), block_pos.as_uvec3())
    }
}
