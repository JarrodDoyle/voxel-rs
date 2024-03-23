use crate::gfx::{BulkBufferBuilder, Context};

use super::brickmap::Brickmap;

#[derive(Debug, Default, Copy, Clone)]
pub struct BrickmapCacheEntry {
    pub grid_idx: usize,
    pub shading_table_offset: u32,
}

#[derive(Debug)]
pub struct BrickmapCache {
    buffer: wgpu::Buffer,
    cache: Vec<Option<BrickmapCacheEntry>>,
    pub index: usize,
    pub num_loaded: u32,
}

impl BrickmapCache {
    pub fn new(context: &Context, size: usize) -> Self {
        let buffer_data = vec![Brickmap::default(); size];
        let buffer = BulkBufferBuilder::new()
            .set_usage(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
            .with_init_buffer_bm("Brickmap Cache", &buffer_data)
            .build(context)
            .remove(0);

        Self {
            buffer,
            cache: vec![None; size],
            index: 0,
            num_loaded: 0,
        }
    }

    pub fn get_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Adds a brickmap entry and returns the entry that was overwritten.
    pub fn add_entry(
        &mut self,
        grid_idx: usize,
        shading_table_offset: u32,
    ) -> Option<BrickmapCacheEntry> {
        // We do this first because we want this to be the index of the most recently added entry
        // This has the side effect of meaning that on the first loop through the cache the first
        // entry is empty, but it's fine.
        self.index = (self.index + 1) % self.cache.len();

        let existing_entry = self.cache[self.index];
        if existing_entry.is_none() {
            self.num_loaded += 1;
        }

        self.cache[self.index] = Some(BrickmapCacheEntry {
            grid_idx,
            shading_table_offset,
        });

        existing_entry
    }

    /// Remove an entry from the cache and return it
    pub fn remove_entry(&mut self, index: usize) -> Option<BrickmapCacheEntry> {
        let entry = self.cache[index];
        if entry.is_some() {
            self.cache[index] = None;
            self.num_loaded -= 1;
        }

        entry
    }

    pub fn get_entry(&self, index: usize) -> Option<BrickmapCacheEntry> {
        self.cache[index]
    }
}
