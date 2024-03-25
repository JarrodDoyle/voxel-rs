use std::collections::HashSet;

use crate::{
    gfx::{BulkBufferBuilder, Context},
    math,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrickgridFlag {
    Empty = 0,
    Unloaded = 1,
    Loading = 2,
    Loaded = 4,
}

impl From<u32> for BrickgridFlag {
    fn from(value: u32) -> Self {
        match value {
            x if x == Self::Unloaded as u32 => Self::Unloaded,
            x if x == Self::Loading as u32 => Self::Loading,
            x if x == Self::Loaded as u32 => Self::Loaded,
            _ => Self::Empty,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BrickgridElement(pub u32);

impl BrickgridElement {
    pub fn new(brickmap_cache_idx: usize, flag: BrickgridFlag) -> Self {
        Self(((brickmap_cache_idx as u32) << 8) + flag as u32)
    }

    pub fn get_pointer(&self) -> usize {
        (self.0 >> 8) as usize
    }

    pub fn get_flag(&self) -> BrickgridFlag {
        BrickgridFlag::from(self.0 & 0xF)
    }
}

#[derive(Debug)]
pub struct Brickgrid {
    dimensions: glam::UVec3,
    data: Vec<BrickgridElement>,
    staged: HashSet<usize>,
    max_upload_count: usize,
    buffer: wgpu::Buffer,
    upload_buffer: wgpu::Buffer,
}

impl Brickgrid {
    pub fn new(context: &Context, dimensions: glam::UVec3, max_upload_count: usize) -> Self {
        let element_count = (dimensions.x * dimensions.y * dimensions.z) as usize;
        let data = vec![BrickgridElement::new(0, BrickgridFlag::Unloaded); element_count];

        // TODO: change type of upload data. Will need some messyness with bytemucking probably
        // but should lead to clearer data definitions
        let mut upload_data = vec![0u32; 4 + 4 * max_upload_count];
        upload_data[0] = max_upload_count as u32;

        let mut buffers = BulkBufferBuilder::new()
            .set_usage(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
            .with_init_buffer_bm("Brickgrid", &data)
            .with_init_buffer_bm("Brickgrid Upload", &upload_data)
            .build(context);

        Self {
            dimensions,
            data,
            staged: HashSet::new(),
            max_upload_count,
            buffer: buffers.remove(0),
            upload_buffer: buffers.remove(0),
        }
    }

    /// Panics if position maps to out of range index
    // pub fn set(&mut self, pos: glam::UVec3, value: BrickgridElement) -> BrickgridElement {
    //      let index = math::to_1d_index(pos, self.dimensions);
    pub fn set(&mut self, index: usize, value: BrickgridElement) -> BrickgridElement {
        let current = self.data[index];
        self.data[index] = value;
        self.staged.insert(index);
        current
    }

    /// Panics if position maps to out of range index
    // pub fn get(&mut self, pos: glam::UVec3) -> BrickgridElement {
    //     let index = math::to_1d_index(pos, self.dimensions);
    pub fn get(&mut self, index: usize) -> BrickgridElement {
        self.data[index]
    }

    pub fn upload(&mut self, context: &Context) {
        let mut upload_data = Vec::new();
        let mut idx = 0;
        self.staged.retain(|e| {
            // We have a limit of how many elements to upload each frame. So we need
            // to keep any excess
            if idx >= self.max_upload_count {
                return true;
            }

            // Index of the brickgrid element, and the value of it
            upload_data.push(*e as u32);
            upload_data.push(self.data[*e].0);

            idx += 1;
            false
        });

        // Upload buffer is {max_count, count, pad, pad, bricks[]}. So we need to add
        // the count and pads, and upload at an offset to skip max_count
        let data = [&[upload_data.len() as u32, 0, 0], &upload_data[..]].concat();
        context
            .queue
            .write_buffer(&self.upload_buffer, 4, bytemuck::cast_slice(&data));

        if idx != 0 {
            log::info!(
                "Uploading {} brickgrid entries. ({} remaining)",
                idx,
                self.staged.len()
            );
        }
    }

    pub fn get_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn get_upload_buffer(&self) -> &wgpu::Buffer {
        &self.upload_buffer
    }
}
