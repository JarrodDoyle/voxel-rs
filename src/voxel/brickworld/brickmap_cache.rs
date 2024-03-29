use crate::gfx::{BulkBufferBuilder, Context};

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Brickmap {
    pub bitmask: [u32; 16],
    pub shading_table_offset: u32,
    pub lod_color: u32,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct BrickmapCacheEntry {
    pub grid_idx: usize,
    pub shading_table_offset: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BrickmapUploadElement {
    cache_idx: u32, // TODO: Change to usize?
    brickmap: Brickmap,
    shading_element_count: u32,
    shading_elements: [u32; 512], // TODO: Replace u32 with custom type?
}

#[derive(Debug)]
pub struct BrickmapCache {
    cache: Vec<Option<BrickmapCacheEntry>>,
    pub index: usize,
    pub num_loaded: u32,
    staged: Vec<BrickmapUploadElement>,
    max_upload_count: usize,
    buffer: wgpu::Buffer,
    upload_buffer: wgpu::Buffer,
}

impl BrickmapCache {
    pub fn new(context: &Context, size: usize, max_upload_count: usize) -> Self {
        let data = vec![Brickmap::default(); size];

        // TODO: change type of upload data. Will need some messyness with bytemucking probably
        // but should lead to clearer data definitions
        let mut upload_data = vec![0u32; 4 + 532 * max_upload_count];
        upload_data[0] = max_upload_count as u32;

        let mut buffers = BulkBufferBuilder::new()
            .set_usage(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST)
            .with_init_buffer_bm("Brickmap Cache", &data)
            .with_init_buffer_bm("Brickmap Unpack", &upload_data)
            .build(context);

        Self {
            cache: vec![None; size],
            index: 0,
            num_loaded: 0,
            staged: vec![],
            max_upload_count,
            buffer: buffers.remove(0),
            upload_buffer: buffers.remove(0),
        }
    }

    pub fn get_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn get_upload_buffer(&self) -> &wgpu::Buffer {
        &self.upload_buffer
    }

    /// Adds a brickmap entry and returns the entry that was overwritten.
    pub fn add_entry(
        &mut self,
        grid_idx: usize,
        shading_table_offset: u32,
        bitmask: [u32; 16],
        albedo_data: Vec<u32>,
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

        // Need to stage this entry
        let brickmap = Brickmap {
            bitmask,
            shading_table_offset,
            lod_color: 0,
        };

        let shading_element_count = albedo_data.len();
        let mut shading_elements = [0u32; 512];
        shading_elements[..shading_element_count].copy_from_slice(&albedo_data);

        let staged_brickmap = BrickmapUploadElement {
            cache_idx: self.index as u32,
            brickmap,
            shading_element_count: shading_element_count as u32,
            shading_elements,
        };
        self.staged.push(staged_brickmap);

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

    pub fn upload(&mut self, context: &Context) {
        // Takes up to max_upload_count upload elements
        let count = usize::min(self.max_upload_count, self.staged.len());
        let iter = self.staged.drain(0..count);
        let upload_data = iter.as_slice();

        // Upload buffer is {max_count, count, pad, pad, maps[]}. So we need to add
        // the count and pads, and upload at an offset to skip max_count
        let data: Vec<u8> = [
            bytemuck::cast_slice(&[count as u32, 0, 0]),
            bytemuck::cast_slice(upload_data),
        ]
        .concat();
        context.queue.write_buffer(&self.upload_buffer, 4, &data);
        drop(iter);

        if count > 0 {
            log::info!(
                "Uploading {} brickmap entries. ({} remaining)",
                count,
                self.staged.len()
            );
        }
    }
}
