use crate::{
    gfx::{self, BufferExt},
    math,
    voxel::world::WorldManager,
};

use super::{
    brickgrid::{Brickgrid, BrickgridElement, BrickgridFlag},
    brickmap_cache::BrickmapCache,
    shading_table::ShadingTableAllocator,
};

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct WorldState {
    brickgrid_dims: [u32; 3],
    _pad: u32,
}

#[derive(Debug)]
pub struct BrickmapManager {
    state_uniform: WorldState,
    state_buffer: wgpu::Buffer,
    brickgrid: Brickgrid,
    brickmap_cache: BrickmapCache,
    shading_table_buffer: wgpu::Buffer,
    shading_table_allocator: ShadingTableAllocator,
    feedback_buffer: wgpu::Buffer,
    feedback_result_buffer: wgpu::Buffer,
    unpack_max_count: usize,
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

        let brickgrid = Brickgrid::new(context, brickgrid_dims, max_uploaded_brickmaps as usize);
        let brickmap_cache = BrickmapCache::new(
            context,
            brickmap_cache_size,
            max_uploaded_brickmaps as usize,
        );

        let shading_table_allocator = ShadingTableAllocator::new(4, shading_table_bucket_size);
        let shading_table = vec![0u32; shading_table_allocator.total_elements as usize];

        let mut feedback_data = vec![0u32; 4 + 4 * max_requested_brickmaps as usize];
        feedback_data[0] = max_requested_brickmaps;
        let feedback_data_u8 = bytemuck::cast_slice(&feedback_data);

        let mut brickmap_upload_data = vec![0u32; 4 + 532 * max_uploaded_brickmaps as usize];
        brickmap_upload_data[0] = max_uploaded_brickmaps;

        let mut buffers = gfx::BulkBufferBuilder::new()
            .with_init_buffer_bm("Brick World State", &[state_uniform])
            .set_usage(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
            .with_init_buffer_bm("Shading Table", &shading_table)
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

            state_buffer: buffers.remove(0),
            shading_table_buffer: buffers.remove(0),
            feedback_buffer: buffers.remove(0),
            feedback_result_buffer: buffers.remove(0),
        }
    }

    pub fn get_brickgrid_buffer(&self) -> &wgpu::Buffer {
        self.brickgrid.get_buffer()
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
        self.brickmap_cache.get_upload_buffer()
    }

    pub fn get_brickgrid_unpack_buffer(&self) -> &wgpu::Buffer {
        self.brickgrid.get_upload_buffer()
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

        let mut brickgrid_element = BrickgridElement::default();

        // We have voxel data so we have a brickmap to upload
        if !albedo_data.is_empty() {
            let shading_idx = self
                .shading_table_allocator
                .try_alloc(albedo_data.len() as u32)
                .unwrap() as usize;

            if let Some(entry) = self.brickmap_cache.add_entry(
                grid_idx,
                shading_idx as u32,
                bitmask_data,
                albedo_data,
            ) {
                // An entry got removed so we need to deallocate it's shading table elements
                // and mark the relevant brickgrid as unloaded
                if let Err(e) = self
                    .shading_table_allocator
                    .try_dealloc(entry.shading_table_offset)
                {
                    log::warn!("{}", e)
                }
                self.brickgrid.set(
                    entry.grid_idx,
                    BrickgridElement::new(0, BrickgridFlag::Unloaded),
                );
            }

            brickgrid_element =
                BrickgridElement::new(self.brickmap_cache.index, BrickgridFlag::Loaded);
        }

        let old = self.brickgrid.set(grid_idx, brickgrid_element);
        if old.get_flag() == BrickgridFlag::Loaded {
            // The brickgrid element was previously loaded so we need to unload any of
            // the data that was associated with it
            if let Some(entry) = self.brickmap_cache.remove_entry(old.get_pointer()) {
                if entry.grid_idx != grid_idx {
                    log::error!(
                        "Mismatch between brickgrid index and brickmap grid index: {} vs {}",
                        grid_idx,
                        entry.grid_idx
                    );
                }

                // We need to deallocate the removed entries shading table elements
                if let Err(e) = self
                    .shading_table_allocator
                    .try_dealloc(entry.shading_table_offset)
                {
                    log::warn!("{}", e)
                }
            }
        }
    }

    fn upload_unpack_buffers(&mut self, context: &gfx::Context) {
        self.brickgrid.upload(context);
        self.brickmap_cache.upload(context);
    }
}
