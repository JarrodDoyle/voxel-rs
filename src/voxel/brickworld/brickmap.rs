use std::collections::HashSet;

use crate::{
    gfx::{self, BufferExt},
    math,
    voxel::world::WorldManager,
};

use super::{
    brickmap_cache::{BrickmapCache, BrickmapCacheEntry},
    shading_table::ShadingTableAllocator,
};

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Brickmap {
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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BrickmapUnpackElement {
    cache_idx: u32,
    brickmap: Brickmap,
    shading_element_count: u32,
    shading_elements: [u32; 512],
}

pub enum BrickgridFlag {
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
    brickmap_cache: BrickmapCache,
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

        let brickmap_cache = BrickmapCache::new(context, brickmap_cache_size);

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
            brickmap_cache,
            shading_table_allocator,
            unpack_max_count: max_uploaded_brickmaps as usize,
            brickgrid_staged,
            brickmap_staged,

            state_buffer: buffers.remove(0),
            brickgrid_buffer: buffers.remove(0),
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
        self.brickmap_cache.get_buffer()
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
        let data: Vec<u32> = self.feedback_result_buffer.get_mapped_range(context, 0..16);
        let request_count = data[1] as usize;

        if request_count > 0 {
            // Reset the request count for next frame
            context
                .queue
                .write_buffer(&self.feedback_buffer, 4, &[0, 0, 0, 0]);

            let range = 16..(16 + 16 * request_count as u64);
            let data = self.feedback_result_buffer.get_mapped_range(context, range);
            for i in 0..request_count {
                let request_data = &data[(i * 4)..(i * 4 + 3)];
                self.handle_request(world, request_data);
            }
        }

        // TODO: Why do we call this here rather than doing it outside of here?
        self.upload_unpack_buffers(context);

        log::info!("Num loaded brickmaps: {}", self.brickmap_cache.num_loaded);
    }

    fn handle_request(&mut self, world: &mut WorldManager, data: &[u32]) {
        let grid_dims = self.state_uniform.brickgrid_dims;

        // Extract brickgrid position of the requested brickmap
        let grid_pos = glam::uvec3(data[0], data[1], data[2]);
        let grid_idx = math::to_1d_index(
            grid_pos,
            glam::uvec3(grid_dims[0], grid_dims[1], grid_dims[2]),
        );

        // We only want to upload voxels that are on the surface, so we cull anything
        // that is surrounded by solid voxels
        let grid_pos = grid_pos.as_ivec3();
        let (bitmask_data, albedo_data) = super::util::cull_interior_voxels(world, grid_pos);

        // If there's no voxel colour data post-culling it means the brickmap is
        // empty. We don't need to upload it, just mark the relevant brickgrid entry.
        if albedo_data.is_empty() {
            if let Some(entry) = self.update_brickgrid_element(grid_idx, 0) {
                // The brickgrid element had a brickmap entry so we need to unload it's
                // shading data
                if let Err(e) = self
                    .shading_table_allocator
                    .try_dealloc(entry.shading_table_offset)
                {
                    log::warn!("{}", e)
                }
            }
            return;
        }

        // Update the shading table
        let shading_idx = self
            .shading_table_allocator
            .try_alloc(albedo_data.len() as u32)
            .unwrap() as usize;

        if let Some(entry) = self.brickmap_cache.add_entry(grid_idx, shading_idx as u32) {
            self.update_brickgrid_element(entry.grid_idx, 1);
        }

        // Update the brickgrid index
        if let Some(old_entry) = self.update_brickgrid_element(
            grid_idx,
            super::util::to_brickgrid_element(
                self.brickmap_cache.index as u32,
                BrickgridFlag::Loaded,
            ),
        ) {
            // The brickgrid element had a brickmap entry so we need to unload it's
            // shading data
            if let Err(e) = self
                .shading_table_allocator
                .try_dealloc(old_entry.shading_table_offset)
            {
                log::warn!("{}", e)
            }
        }

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
            cache_idx: self.brickmap_cache.index as u32,
            brickmap,
            shading_element_count: shading_element_count as u32,
            shading_elements,
        };
        self.brickmap_staged.push(staged_brickmap);
    }

    fn update_brickgrid_element(&mut self, index: usize, data: u32) -> Option<BrickmapCacheEntry> {
        let mut brickmap_cache_entry = None;
        if (self.brickgrid[index] & 0xF) == 4 {
            let cache_index = (self.brickgrid[index] >> 8) as usize;
            brickmap_cache_entry = self.brickmap_cache.get_entry(cache_index);
        }

        // We're safe to overwrite the CPU brickgrid and mark for GPU upload now
        self.brickgrid[index] = data;
        self.brickgrid_staged.insert(index);
        brickmap_cache_entry
    }

    // TODO: Tidy this up more
    fn upload_unpack_buffers(&mut self, context: &gfx::Context) {
        // Brickgrid
        let mut data = Vec::new();
        let mut iter = self.brickgrid_staged.iter();
        let mut to_remove = Vec::new();
        for _ in 0..self.unpack_max_count {
            match iter.next() {
                Some(val) => {
                    to_remove.push(*val);
                    data.push(*val as u32);
                    data.push(self.brickgrid[*val]);
                }
                None => break,
            }
        }
        for val in &to_remove {
            self.brickgrid_staged.remove(val);
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
            bytemuck::cast_slice(&[&[data.len() as u32, 0, 0], &data[..]].concat()),
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
}
